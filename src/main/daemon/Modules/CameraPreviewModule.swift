import AVFoundation
import Cocoa
import Foundation
import os
import QuartzCore

private struct CameraPreviewSource: Equatable {
    var deviceId: String?
    var deviceName: String?
    var resolution = "720p"
}

class CameraPreviewModule: Module {
    let name = DaemonContract.CameraPreview.module
    
    private var panel: NSPanel?
    private var contentView: CameraPreviewContentView?
    private var moveObserver: NSObjectProtocol?
    
    private var source = CameraPreviewSource()
    private var flipped: Bool = false
    private var isContentProtected: Bool = false
    private var presentationGeneration = WindowPresentationGeneration()

    deinit { stopMoveTracking() }
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.CameraPreview.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .show:
            handleShow(params: params, requestId: requestId)
        case .hide:
            handleHide(requestId: requestId)
        case .update:
            handleUpdate(params: params, requestId: requestId)
        case .setContentProtection:
            handleSetContentProtection(params: params, requestId: requestId)
        }
    }
    
    private func handleShow(params: [String: AnyCodable]?, requestId: String) {
        let source = CameraPreviewSource(
            deviceId: params?["deviceId"]?.string(),
            deviceName: params?["deviceName"]?.string(),
            resolution: params?["resolution"]?.string() ?? "720p"
        )
        let newFlipped = params?["flipped"]?.bool() ?? false
        
        let x = params?["x"]?.int()
        let y = params?["y"]?.int()
        
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                do {
                    self.source = source
                    self.flipped = newFlipped
                    let visible = try await self.showPanel(x: x, y: y)
                    #if RECORDING_OUTLINE_TEST
                    self.testShowResults[requestId] = visible
                    #endif
                    self.respond(id: requestId, result: ["visible": visible])
                } catch {
                    #if RECORDING_OUTLINE_TEST
                    self.testFailedShows.insert(requestId)
                    #endif
                    self.respondError(id: requestId, code: "CAPTURE_ERROR", message: error.localizedDescription)
                }
            }
        }
    }
    
    private func handleHide(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                self.hidePanel()
                self.respond(id: requestId, result: ["visible": false])
                #if RECORDING_OUTLINE_TEST
                self.testHideReplies.append(requestId)
                #endif
                try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            }
        }
    }
    
    private func handleUpdate(params: [String: AnyCodable]?, requestId: String) {
        let hasDeviceId = params?["deviceId"] != nil
        let hasDeviceName = params?["deviceName"] != nil
        let hasResolution = params?["resolution"] != nil
        let deviceId = params?["deviceId"]?.string()
        let deviceName = params?["deviceName"]?.string()
        let resolution = params?["resolution"]?.string()
        let flipped = params?["flipped"]?.bool()
        let x = params?["x"]?.int()
        let y = params?["y"]?.int()

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            if hasDeviceId {
                self.source.deviceId = deviceId
            }
            if hasDeviceName {
                self.source.deviceName = deviceName
            }
            if let resolution {
                self.source.resolution = resolution
            }
            if let flipped {
                self.flipped = flipped
                self.contentView?.updateFlipped(flipped)
            }
            if let x, let y {
                self.updatePosition(x: x, y: y)
            }
            if self.panel?.alphaValue == 1 && (hasDeviceId || hasDeviceName || hasResolution) {
                self.contentView?.updateCamera(self.source)
            }
            self.respond(id: requestId, result: ["updated": true])
        }
    }
    
    private func handleSetContentProtection(params: [String: AnyCodable]?, requestId: String) {
        guard let enabled = params?["enabled"]?.bool() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "enabled is required")
            return
        }
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.isContentProtected = enabled
            self.panel?.sharingType = enabled ? .none : .readOnly
            #if RECORDING_OUTLINE_TEST
            self.testProtectionReplies.append(enabled)
            #endif
            self.respond(id: requestId, result: ["protected": enabled])
        }
    }
    
    @MainActor
    private func showPanel(x: Int?, y: Int?) async throws -> Bool {
        let generation = presentationGeneration.begin()
        await RecordingCaptureWindowRegistry.shared.waitForCaptureStartIfNeeded()
        guard presentationGeneration.isCurrent(generation) else { return false }
        if let panel {
            if panel.alphaValue == 0 {
                do {
                    try await RecordingCaptureWindowRegistry.shared.waitForWindowPresentation()
                } catch {
                    guard presentationGeneration.isCurrent(generation), self.panel === panel else { return false }
                    stopMoveTracking()
                    panel.orderOut(nil)
                    RecordingCaptureWindowRegistry.shared.unregister(panel)
                    contentView?.stopCamera()
                    self.panel = nil
                    self.contentView = nil
                    try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
                    throw error
                }
                guard presentationGeneration.isCurrent(generation), self.panel === panel else { return false }
                panel.alphaValue = 1
                panel.ignoresMouseEvents = false
            }
            setupMoveTracking(for: panel)
            contentView?.updateFlipped(flipped)
            if let x = x, let y = y {
                updatePosition(x: x, y: y)
            }
            if let contentView, contentView.source != source {
                contentView.updateCamera(source)
            }
            panel.orderFrontRegardless()
            return true
        }
        
        let previewSize: CGFloat = 230
        let shadowPadding: CGFloat = 20
        let totalSize = previewSize + shadowPadding * 2
        let size = NSSize(width: totalSize, height: totalSize)
        let position = calculatePosition(x: x, y: y, size: size)
        
        let panel = NSPanel(
            contentRect: NSRect(origin: position, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        
        panel.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.maximumWindow)) + 1)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        panel.sharingType = isContentProtected ? .none : .readOnly
        panel.alphaValue = 0
        panel.ignoresMouseEvents = true

        let contentView = CameraPreviewContentView(
            frame: NSRect(origin: .zero, size: size),
            flipped: flipped,
            padding: shadowPadding
        )
        
        panel.contentView = contentView
        self.panel = panel
        self.contentView = contentView
        panel.orderFrontRegardless()
        guard RecordingCaptureWindowRegistry.shared.register(panel) != nil else {
            panel.orderOut(nil)
            contentView.stopCamera()
            self.panel = nil
            self.contentView = nil
            throw RecordingCaptureFilterError.unavailableWindowNumber
        }
        do {
            try await RecordingCaptureWindowRegistry.shared.waitForWindowPresentation()
        } catch {
            guard presentationGeneration.isCurrent(generation), self.panel === panel else { return false }
            RecordingCaptureWindowRegistry.shared.unregister(panel)
            contentView.stopCamera()
            panel.orderOut(nil)
            self.panel = nil
            self.contentView = nil
            try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            throw error
        }
        guard presentationGeneration.isCurrent(generation), self.panel === panel else {
            if self.panel !== panel {
                panel.orderOut(nil)
                RecordingCaptureWindowRegistry.shared.unregister(panel)
                contentView.stopCamera()
                try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
            }
            return false
        }
        panel.alphaValue = 1
        panel.ignoresMouseEvents = false
        
        setupMoveTracking(for: panel)
        contentView.updateCamera(source)
        return true
    }
    
    @MainActor
    private func hidePanel() {
        presentationGeneration.cancel()
        stopMoveTracking()
        let contentView = self.contentView
        let panel = self.panel
        self.panel = nil
        self.contentView = nil
        panel?.alphaValue = 0
        if panel != nil { CATransaction.flush() }
        contentView?.stopCamera()
        panel?.orderOut(nil)
        if let panel {
            RecordingCaptureWindowRegistry.shared.unregister(panel)
        }
    }
    
    @MainActor
    private func updatePosition(x: Int, y: Int) {
        guard let panel = panel else { return }
        panel.setFrameOrigin(calculatePosition(x: x, y: y, size: panel.frame.size))
    }
    
    @MainActor
    private func calculatePosition(x: Int?, y: Int?, size: NSSize) -> NSPoint {
        let screens = NSScreen.screens
        return Self.fittedPanelOrigin(
            x: x, y: y, size: size,
            primaryHeight: screens.first?.frame.height ?? 0,
            displays: screens.map { (frame: $0.frame, visibleFrame: $0.visibleFrame) },
            fallback: (panel?.screen ?? NSScreen.main ?? screens.first)?.visibleFrame
        )
    }

    static func fittedPanelOrigin(
        x: Int?, y: Int?, size: NSSize, primaryHeight: CGFloat,
        displays: [(frame: NSRect, visibleFrame: NSRect)],
        fallback: NSRect?
    ) -> NSPoint {
        func constrained(_ origin: NSPoint, to visible: NSRect) -> NSPoint {
            NSPoint(
                x: min(max(origin.x, visible.minX), max(visible.minX, visible.maxX - size.width)),
                y: min(max(origin.y, visible.minY), max(visible.minY, visible.maxY - size.height))
            )
        }
        let topLeft = x.flatMap { x in
            y.map { y in
                NSPoint(
                    x: CGFloat(x),
                    y: DisplayCoordinates.cocoaY(topLeftY: CGFloat(y), height: 0, primaryHeight: primaryHeight)
                )
            }
        }
        let requestedOrigin = topLeft.map { NSPoint(x: $0.x, y: $0.y - size.height) }
        if let topLeft, let requestedOrigin,
           let display = displays.first(where: {
               topLeft.x >= $0.frame.minX && topLeft.x < $0.frame.maxX
                   && topLeft.y > $0.frame.minY && topLeft.y <= $0.frame.maxY
           }) {
            return constrained(requestedOrigin, to: display.visibleFrame)
        }
        guard let fallback else { return requestedOrigin ?? NSPoint(x: 100, y: 100) }
        let margin: CGFloat = 32
        let defaultOrigin = NSPoint(
            x: fallback.maxX - size.width - margin,
            y: fallback.minY + margin
        )
        return constrained(defaultOrigin, to: fallback)
    }
    
    @MainActor
    private func setupMoveTracking(for panel: NSPanel) {
        guard moveObserver == nil else { return }
        moveObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didMoveNotification,
            object: panel,
            queue: .main
        ) { [weak self, weak panel] _ in
            Task { @MainActor in
                guard let self, let panel, self.panel === panel else { return }

                let x = Int(panel.frame.origin.x)
                let y = Int(DisplayCoordinates.topLeftY(
                    cocoaY: panel.frame.origin.y, height: panel.frame.height,
                    primaryHeight: DisplayCoordinates.primaryHeight
                ))

                #if RECORDING_OUTLINE_TEST
                self.testReportedPositions.append(NSPoint(x: x, y: y))
                #endif
                self.emit(event: "position-changed", data: ["x": x, "y": y])
            }
        }
    }

    private func stopMoveTracking() {
        guard let moveObserver else { return }
        NotificationCenter.default.removeObserver(moveObserver)
        self.moveObserver = nil
    }

    #if RECORDING_OUTLINE_TEST
    @MainActor
    static func testCameraStartupQueue() -> [String] {
        let view = CameraPreviewContentView(frame: NSRect(x: 0, y: 0, width: 270, height: 270),
                                            flipped: false, padding: 20)
        return view.testStartupQueue()
    }

    @MainActor
    static func testCameraCompletionOrdering() -> [Bool] {
        let view = CameraPreviewContentView(frame: NSRect(x: 0, y: 0, width: 270, height: 270),
                                            flipped: false, padding: 20)
        view.updateCamera(CameraPreviewSource(deviceName: "A"))
        let first = view.testPreviewCompletion()
        view.updateCamera(CameraPreviewSource(deviceName: "B"))
        let second = view.testPreviewCompletion()
        view.updateCamera(CameraPreviewSource(deviceName: "A"))
        let latest = view.testPreviewCompletion()
        let results = [first(), second(), latest()]
        let pending = view.testPreviewCompletion()
        view.stopCamera()
        return results + [pending()]
    }

    @MainActor
    static func testErrorView(messages: [String], retry: Bool = false) -> NSView {
        let content = CameraPreviewContentView(frame: NSRect(x: 0, y: 0, width: 270, height: 270),
                                               flipped: false, padding: 20)
        messages.forEach { content.showError($0) }
        if retry {
            content.updateCamera(CameraPreviewSource(deviceName: "generated camera"))
        }
        return content
    }

    private(set) var testReportedPositions: [NSPoint] = []
    private(set) var testShowResults: [String: Bool] = [:]
    private(set) var testFailedShows: Set<String> = []
    private(set) var testHideReplies: [String] = []
    private(set) var testProtectionReplies: [Bool] = []

    @MainActor
    var testStartup: (panel: NSPanel?, generation: UInt64, starts: [String?]) {
        (panel, presentationGeneration.value, contentView?.testStartedCameras ?? [])
    }

    @MainActor
    func testStartedCameras(in panel: NSPanel) -> [String?]? {
        (panel.contentView as? CameraPreviewContentView)?.testStartedCameras
    }

    @MainActor
    func testInstallPanel(_ panel: NSPanel) -> Bool {
        self.panel = panel
        guard RecordingCaptureWindowRegistry.shared.register(panel) != nil else { return false }
        setupMoveTracking(for: panel)
        return true
    }

    @MainActor
    func testHidePanel() async {
        hidePanel()
        try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
    }
    #endif
}

private class CameraPreviewContentView: NSView {
    private var captureSession: AVCaptureSession?
    private var previewLayer: AVCaptureVideoPreviewLayer?
    private var containerView: NSView!
    private var errorLabel: NSTextField?
    
    private(set) var source: CameraPreviewSource?
    private var isMirrored: Bool
    private let padding: CGFloat
    private let cornerRadius: CGFloat = 65
    
    private let cameraQueue = DispatchQueue(label: "com.porabuild.poratake.camera-preview")
    private let cameraGeneration = OSAllocatedUnfairLock(initialState: WindowPresentationGeneration())

    #if RECORDING_OUTLINE_TEST
    private(set) var testStartedCameras: [String?] = []
    private var testCameraStartWork: ((String?) -> Void)?
    #endif
    
    init(frame: NSRect, flipped: Bool, padding: CGFloat) {
        self.isMirrored = flipped
        self.padding = padding
        super.init(frame: frame)
        setupView()
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    private func setupView() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        
        let contentRect = NSRect(
            x: padding,
            y: padding,
            width: bounds.width - padding * 2,
            height: bounds.height - padding * 2
        )
        
        containerView = NSView(frame: contentRect)
        containerView.wantsLayer = true
        containerView.layer?.cornerRadius = cornerRadius
        containerView.layer?.masksToBounds = true
        containerView.layer?.backgroundColor = NSColor.black.cgColor
        
        let shadowView = NSView(frame: contentRect)
        shadowView.wantsLayer = true
        shadowView.layer?.cornerRadius = cornerRadius
        shadowView.layer?.shadowColor = NSColor.black.cgColor
        shadowView.layer?.shadowOffset = CGSize(width: 0, height: -4)
        shadowView.layer?.shadowRadius = 10
        shadowView.layer?.shadowOpacity = 0.4
        shadowView.layer?.backgroundColor = NSColor.black.cgColor
        
        addSubview(shadowView)
        addSubview(containerView)
    }
    
    private func startCamera(_ source: CameraPreviewSource, generation: UInt64) {
        #if RECORDING_OUTLINE_TEST
        testStartedCameras.append(source.deviceName)
        alphaValue = 0
        let testCameraStartWork = self.testCameraStartWork
        #endif
        cameraQueue.async { [weak self] in
            guard let self, self.isCurrentCamera(generation) else { return }

            #if RECORDING_OUTLINE_TEST
            testCameraStartWork?(source.deviceName)
            #else
            self.stopCaptureSession()
            guard self.isCurrentCamera(generation) else { return }

            guard let camera = CameraDeviceLookup.find(deviceId: source.deviceId, deviceName: source.deviceName) else {
                self.reportCameraError("No camera found", generation: generation)
                return
            }

            let session = AVCaptureSession()
            session.sessionPreset = .high
            do {
                let input = try AVCaptureDeviceInput(device: camera)
                if session.canAddInput(input) {
                    session.addInput(input)
                }
            } catch {
                self.reportCameraError("Camera access denied", generation: generation)
                return
            }

            guard self.isCurrentCamera(generation) else { return }
            session.startRunning()
            self.captureSession = session
            guard self.isCurrentCamera(generation) else {
                self.stopCaptureSession()
                return
            }
            DispatchQueue.main.async { [weak self] in
                self?.installPreview(session, generation: generation)
            }
            #endif
        }
    }

    private func isCurrentCamera(_ generation: UInt64) -> Bool {
        cameraGeneration.withLock { $0.isCurrent(generation) }
    }

    private func reportCameraError(_ message: String, generation: UInt64) {
        DispatchQueue.main.async { [weak self] in
            guard let self, self.isCurrentCamera(generation) else { return }
            self.showError(message)
        }
    }

    private func installPreview(_ session: AVCaptureSession, generation: UInt64) {
        guard isCurrentCamera(generation) else { return }
        clearError()
        clearPreview()

        let previewLayer = AVCaptureVideoPreviewLayer(session: session)
        previewLayer.videoGravity = .resizeAspectFill
        previewLayer.frame = containerView.bounds
        previewLayer.setAffineTransform(CGAffineTransform(scaleX: isMirrored ? -1 : 1, y: 1))
        containerView.layer?.addSublayer(previewLayer)
        self.previewLayer = previewLayer
    }

    #if RECORDING_OUTLINE_TEST
    func testStartupQueue() -> [String] {
        var started: [String] = []
        testCameraStartWork = { started.append($0 ?? "") }
        let release = DispatchSemaphore(value: 0)
        cameraQueue.async { release.wait() }
        for name in ["A", "B", "A"] {
            updateCamera(CameraPreviewSource(deviceName: name))
        }
        release.signal()
        cameraQueue.sync {}

        cameraQueue.async { release.wait() }
        updateCamera(CameraPreviewSource(deviceName: "closed"))
        stopCamera()
        release.signal()
        cameraQueue.sync {}
        return started
    }

    func testPreviewCompletion() -> () -> Bool {
        let session = AVCaptureSession()
        let generation = cameraGeneration.withLock { $0.value }
        return { [self] in
            installPreview(session, generation: generation)
            return previewLayer?.session === session
        }
    }
    #endif

    private func stopCaptureSession() {
        guard let session = captureSession else { return }
        captureSession = nil
        if session.isRunning {
            session.stopRunning()
        }
        for input in session.inputs {
            session.removeInput(input)
        }
        for output in session.outputs {
            session.removeOutput(output)
        }
    }

    private func clearPreview() {
        previewLayer?.removeFromSuperlayer()
        previewLayer = nil
    }

    func stopCamera() {
        cameraGeneration.withLock { $0.cancel() }
        clearPreview()
        cameraQueue.async { self.stopCaptureSession() }
    }

    func updateCamera(_ source: CameraPreviewSource) {
        let generation = cameraGeneration.withLock { $0.begin() }
        self.source = source
        clearError()
        clearPreview()
        startCamera(source, generation: generation)
    }

    private func clearError() {
        errorLabel?.removeFromSuperview()
        errorLabel = nil
    }

    func updateFlipped(_ flipped: Bool) {
        self.isMirrored = flipped
        previewLayer?.setAffineTransform(
            CGAffineTransform(scaleX: flipped ? -1 : 1, y: 1)
        )
    }
    
    func showError(_ message: String) {
        if let errorLabel {
            errorLabel.stringValue = message
            return
        }
        let label = NSTextField(labelWithString: message)
        label.translatesAutoresizingMaskIntoConstraints = false
        label.textColor = NSColor.white.withAlphaComponent(0.9)
        label.font = NSFont.systemFont(ofSize: 12)
        label.alignment = .center
        containerView.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: 12),
            label.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -12),
            label.centerYAnchor.constraint(equalTo: containerView.centerYAnchor),
        ])
        errorLabel = label
    }
    
    override func layout() {
        super.layout()
        
        let contentRect = NSRect(
            x: padding,
            y: padding,
            width: bounds.width - padding * 2,
            height: bounds.height - padding * 2
        )
        
        containerView?.frame = contentRect
        previewLayer?.frame = containerView?.bounds ?? .zero
        
        if let shadowView = subviews.first {
            shadowView.frame = contentRect
        }
    }
}
