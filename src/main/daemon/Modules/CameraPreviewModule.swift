import AVFoundation
import Cocoa
import Foundation

class CameraPreviewModule: Module {
    let name = DaemonContract.CameraPreview.module
    
    private var panel: NSPanel?
    private var contentView: CameraPreviewContentView?
    
    private var deviceId: String?
    private var deviceName: String?
    private var resolution: String = "720p"
    private var flipped: Bool = false
    private var isContentProtected: Bool = false
    private var presentationGeneration = WindowPresentationGeneration()
    
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
        let newDeviceId = params?["deviceId"]?.string()
        let newDeviceName = params?["deviceName"]?.string()
        let newResolution = params?["resolution"]?.string() ?? "720p"
        let newFlipped = params?["flipped"]?.bool() ?? false
        
        let x = params?["x"]?.int()
        let y = params?["y"]?.int()
        
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                do {
                    let deviceChanged = newDeviceId != self.deviceId || newDeviceName != self.deviceName
                    let resolutionChanged = newResolution != self.resolution
                    self.deviceId = newDeviceId
                    self.deviceName = newDeviceName
                    self.resolution = newResolution
                    self.flipped = newFlipped
                    let visible = try await self.showPanel(x: x, y: y)

                    if visible && (deviceChanged || resolutionChanged) && self.contentView != nil {
                        self.contentView?.updateCamera(
                            deviceId: self.deviceId,
                            deviceName: self.deviceName,
                            resolution: self.resolution
                        )
                    }

                    self.respond(id: requestId, result: ["visible": visible])
                } catch {
                    self.respondError(id: requestId, code: "CAPTURE_ERROR", message: error.localizedDescription)
                }
            }
        }
    }
    
    private func handleHide(requestId: String) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                await self.hidePanel()
                self.respond(id: requestId, result: ["visible": false])
            }
        }
    }
    
    private func handleUpdate(params: [String: AnyCodable]?, requestId: String) {
        let deviceChanged = params?["deviceId"] != nil || params?["deviceName"] != nil
        let resolutionChanged = params?["resolution"] != nil
        let hasDeviceId = params?["deviceId"] != nil
        let hasDeviceName = params?["deviceName"] != nil
        let deviceId = params?["deviceId"]?.string()
        let deviceName = params?["deviceName"]?.string()
        let resolution = params?["resolution"]?.string()
        let flipped = params?["flipped"]?.bool()
        let x = params?["x"]?.int()
        let y = params?["y"]?.int()

        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                if hasDeviceId {
                    self.deviceId = deviceId
                }
                if hasDeviceName {
                    self.deviceName = deviceName
                }
                if let resolution {
                    self.resolution = resolution
                }
                if let flipped {
                    self.flipped = flipped
                    self.contentView?.updateFlipped(flipped)
                }
                if let x, let y {
                    self.updatePosition(x: x, y: y)
                }
                if deviceChanged || resolutionChanged {
                    self.contentView?.updateCamera(
                        deviceId: self.deviceId,
                        deviceName: self.deviceName,
                        resolution: self.resolution
                    )
                }
                self.respond(id: requestId, result: ["updated": true])
            }
        }
    }
    
    private func handleSetContentProtection(params: [String: AnyCodable]?, requestId: String) {
        guard let enabled = params?["enabled"]?.bool() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "enabled is required")
            return
        }
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            Task { @MainActor in
                self.isContentProtected = enabled
                self.panel?.sharingType = enabled ? .none : .readOnly
                self.respond(id: requestId, result: ["protected": enabled])
            }
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
                    try await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
                } catch {
                    guard presentationGeneration.isCurrent(generation), self.panel === panel else { return false }
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
            contentView?.updateFlipped(flipped)
            if let x = x, let y = y {
                updatePosition(x: x, y: y)
            }
            panel.orderFrontRegardless()
            return true
        }
        
        let previewSize: CGFloat = 230
        let shadowPadding: CGFloat = 20
        let totalSize = previewSize + shadowPadding * 2
        
        let position = calculatePosition(x: x, y: y, width: totalSize, height: totalSize)
        
        let panel = NSPanel(
            contentRect: NSRect(x: position.x, y: position.y, width: totalSize, height: totalSize),
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
            frame: NSRect(x: 0, y: 0, width: totalSize, height: totalSize),
            deviceId: deviceId,
            deviceName: deviceName,
            resolution: resolution,
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
            try await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
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
        
        setupMoveTracking()
        return true
    }
    
    @MainActor
    private func hidePanel() async {
        presentationGeneration.cancel()
        let contentView = self.contentView
        let panel = self.panel
        self.panel = nil
        self.contentView = nil
        contentView?.stopCamera()
        panel?.orderOut(nil)
        if let panel {
            RecordingCaptureWindowRegistry.shared.unregister(panel)
        }
        try? await RecordingCaptureWindowRegistry.shared.updateFilterIfNeeded()
    }
    
    @MainActor
    private func updatePosition(x: Int, y: Int) {
        guard let panel = panel else { return }
        let mainScreenHeight = NSScreen.main?.frame.height ?? 0
        let cocoaY = mainScreenHeight - CGFloat(y) - panel.frame.height
        panel.setFrameOrigin(NSPoint(x: CGFloat(x), y: cocoaY))
    }
    
    @MainActor
    private func calculatePosition(x: Int?, y: Int?, width: CGFloat, height: CGFloat) -> NSPoint {
        let mainScreenHeight = NSScreen.main?.frame.height ?? 0
        
        if let x = x, let y = y {
            let cocoaY = mainScreenHeight - CGFloat(y) - height
            return NSPoint(x: CGFloat(x), y: cocoaY)
        }
        
        guard let screen = NSScreen.main else {
            return NSPoint(x: 100, y: 100)
        }
        
        let screenWidth = screen.visibleFrame.width
        let screenHeight = screen.visibleFrame.height
        let margin: CGFloat = 32
        
        let defaultX = screenWidth - width - margin + screen.visibleFrame.origin.x
        let defaultY = screen.visibleFrame.origin.y + margin
        
        return NSPoint(x: defaultX, y: defaultY)
    }
    
    @MainActor
    private func setupMoveTracking() {
        NotificationCenter.default.addObserver(
            forName: NSWindow.didMoveNotification,
            object: panel,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor in
                guard let self = self, let panel = self.panel else { return }

                let mainScreenHeight = NSScreen.main?.frame.height ?? 0
                let x = Int(panel.frame.origin.x)
                let y = Int(mainScreenHeight - panel.frame.origin.y - panel.frame.height)

                self.emit(event: "position-changed", data: ["x": x, "y": y])
            }
        }
    }
}

private class CameraPreviewContentView: NSView {
    private var captureSession: AVCaptureSession?
    private var previewLayer: AVCaptureVideoPreviewLayer?
    private var containerView: NSView!
    
    private var deviceId: String?
    private var deviceName: String?
    private var resolution: String
    private var isMirrored: Bool
    private let padding: CGFloat
    private let cornerRadius: CGFloat = 65
    
    private let cameraQueue = DispatchQueue(label: "com.porabuild.poratake.camera-preview")
    private var isStopped: Bool = false
    
    init(frame: NSRect, deviceId: String?, deviceName: String?, resolution: String, flipped: Bool, padding: CGFloat) {
        self.deviceId = deviceId
        self.deviceName = deviceName
        self.resolution = resolution
        self.isMirrored = flipped
        self.padding = padding
        super.init(frame: frame)
        setupView()
        startCamera()
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
    
    func startCamera() {
        cameraQueue.async { [weak self] in
            guard let self = self else { return }
            
            self.stopCameraInternal()
            self.isStopped = false
            
            guard let camera = self.findCamera() else {
                DispatchQueue.main.async { [weak self] in
                    self?.showError("No camera found")
                }
                return
            }
            
            guard !self.isStopped else { return }
            
            let session = AVCaptureSession()
            session.sessionPreset = .high
            
            do {
                let input = try AVCaptureDeviceInput(device: camera)
                if session.canAddInput(input) {
                    session.addInput(input)
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    self?.showError("Camera access denied")
                }
                return
            }
            
            guard !self.isStopped else {
                self.cleanupSession(session)
                return
            }
            
            session.startRunning()
            
            guard !self.isStopped else {
                self.cleanupSession(session)
                return
            }
            
            DispatchQueue.main.async { [weak self] in
                guard let self = self, !self.isStopped else {
                    self?.cleanupSession(session)
                    return
                }
                
                self.captureSession = session
                
                let previewLayer = AVCaptureVideoPreviewLayer(session: session)
                previewLayer.videoGravity = .resizeAspectFill
                previewLayer.frame = self.containerView.bounds
                previewLayer.setAffineTransform(
                    CGAffineTransform(scaleX: self.isMirrored ? -1 : 1, y: 1)
                )
                
                self.containerView.layer?.addSublayer(previewLayer)
                self.previewLayer = previewLayer
            }
        }
    }
    
    private func cleanupSession(_ session: AVCaptureSession) {
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
    
    private func stopCameraInternal() {
        isStopped = true
        
        if let session = captureSession {
            cleanupSession(session)
        }
        captureSession = nil
        
        DispatchQueue.main.async { [weak self] in
            self?.previewLayer?.removeFromSuperlayer()
            self?.previewLayer = nil
        }
    }
    
    func stopCamera() {
        cameraQueue.async { [weak self] in
            self?.stopCameraInternal()
        }
    }
    
    func updateCamera(deviceId: String?, deviceName: String?, resolution: String) {
        self.deviceId = deviceId
        self.deviceName = deviceName
        self.resolution = resolution
        startCamera()
    }

    func updateFlipped(_ flipped: Bool) {
        self.isMirrored = flipped
        previewLayer?.setAffineTransform(
            CGAffineTransform(scaleX: flipped ? -1 : 1, y: 1)
        )
    }
    
    private func findCamera() -> AVCaptureDevice? {
        let discoverySession: AVCaptureDevice.DiscoverySession
        if #available(macOS 14.0, *) {
            discoverySession = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInWideAngleCamera, .external, .continuityCamera],
                mediaType: .video,
                position: .unspecified
            )
        } else {
            discoverySession = AVCaptureDevice.DiscoverySession(
                deviceTypes: [.builtInWideAngleCamera, .externalUnknown],
                mediaType: .video,
                position: .unspecified
            )
        }
        
        if let deviceName = deviceName {
            if let device = discoverySession.devices.first(where: { $0.localizedName == deviceName }) {
                return device
            }
            if let device = discoverySession.devices.first(where: { 
                deviceName.contains($0.localizedName) || $0.localizedName.contains(deviceName) 
            }) {
                return device
            }
        }
        
        if let deviceId = deviceId {
            if let device = discoverySession.devices.first(where: { $0.uniqueID == deviceId }) {
                return device
            }
        }
        
        return AVCaptureDevice.default(for: .video)
    }
    
    private func showError(_ message: String) {
        let label = NSTextField(labelWithString: message)
        label.textColor = .secondaryLabelColor
        label.font = NSFont.systemFont(ofSize: 12)
        label.alignment = .center
        label.frame = containerView.bounds
        containerView.addSubview(label)
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
