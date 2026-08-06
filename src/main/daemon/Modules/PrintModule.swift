import AppKit
import Foundation

class PrintableImageView: NSView {
    var image: NSImage?
    private var scale: CGFloat = 1.0
    private var pageHeight: CGFloat = 0
    
    func configure(with printInfo: NSPrintInfo) {
        guard let image = image else { return }
        
        let paperSize = printInfo.paperSize
        pageHeight = paperSize.height - printInfo.topMargin - printInfo.bottomMargin
        let pageWidth = paperSize.width - printInfo.leftMargin - printInfo.rightMargin
        
        scale = min(pageWidth / image.size.width, 1.0)
        let scaledWidth = image.size.width * scale
        let scaledHeight = image.size.height * scale
        
        self.frame = NSRect(x: 0, y: 0, width: scaledWidth, height: scaledHeight)
    }
    
    override func draw(_ dirtyRect: NSRect) {
        guard let image = image else { return }
        
        NSGraphicsContext.current?.imageInterpolation = .high
        image.draw(in: bounds, from: NSRect(origin: .zero, size: image.size), operation: .sourceOver, fraction: 1.0)
    }
    
    override func knowsPageRange(_ range: NSRangePointer) -> Bool {
        guard let _ = image, pageHeight > 0 else {
            range.pointee = NSRange(location: 1, length: 1)
            return true
        }
        
        let pageCount = max(1, Int(ceil(bounds.height / pageHeight)))
        range.pointee = NSRange(location: 1, length: pageCount)
        return true
    }
    
    override func rectForPage(_ page: Int) -> NSRect {
        let yOffset = CGFloat(page - 1) * pageHeight
        let height = min(pageHeight, bounds.height - yOffset)
        return NSRect(x: 0, y: yOffset, width: bounds.width, height: height)
    }
}

class PrintModule: Module {
    let name = "print"
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "image":
            handlePrintImage(params: params, requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }
    
    private func handlePrintImage(params: [String: AnyCodable]?, requestId: String) {
        guard let imageBase64 = params?["imageBase64"]?.string() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Missing imageBase64 parameter")
            return
        }
        
        guard let imageData = Data(base64Encoded: imageBase64),
              let image = NSImage(data: imageData) else {
            respondError(id: requestId, code: "INVALID_IMAGE", message: "Failed to decode image data")
            return
        }
        
        DispatchQueue.main.async { [weak self] in
            self?.showPrintDialog(for: image, requestId: requestId)
        }
    }
    
    private func showPrintDialog(for image: NSImage, requestId: String) {
        respond(id: requestId, result: ["success": true])
        
        let printInfo = NSPrintInfo.shared.copy() as! NSPrintInfo
        printInfo.horizontalPagination = .clip
        printInfo.verticalPagination = .clip
        printInfo.isHorizontallyCentered = true
        printInfo.isVerticallyCentered = false
        printInfo.topMargin = 36
        printInfo.bottomMargin = 36
        printInfo.leftMargin = 36
        printInfo.rightMargin = 36
        
        let printView = PrintableImageView()
        printView.image = image
        printView.configure(with: printInfo)
        
        let printOperation = NSPrintOperation(view: printView, printInfo: printInfo)
        printOperation.showsPrintPanel = true
        printOperation.showsProgressPanel = true
        
        NSApp.activate(ignoringOtherApps: true)
        
        printOperation.run()
    }
}
