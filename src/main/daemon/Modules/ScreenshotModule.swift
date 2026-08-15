import Cocoa
import Foundation
import ImageIO
import UniformTypeIdentifiers

class ScreenshotModule: Module {
    let name = "screenshot"

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "capture-area":
            handleCaptureArea(params: params, requestId: requestId)
        case "capture-window":
            handleCaptureWindow(params: params, requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }

    private func handleCaptureArea(params: [String: AnyCodable]?, requestId: String) {
        guard
            let path = params?["path"]?.string(),
            let rect = areaRect(params: params)
        else {
            respondError(
                id: requestId,
                code: "INVALID_PARAMS",
                message: "A destination path and a capture area with a positive width and height are required"
            )
            return
        }

        let cached = params?["cached"]?.bool() ?? false

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            let image = (cached ? FrozenFrameStore.crop(to: rect) : nil)
                ?? CGWindowListCreateImage(rect, .optionOnScreenOnly, kCGNullWindowID, [.bestResolution])

            guard let image = image, self.writePng(image, to: path) else {
                self.respondError(id: requestId, code: "CAPTURE_FAILED", message: "Failed to capture the area")
                return
            }

            self.respond(id: requestId, result: [
                "path": path,
                "width": image.width,
                "height": image.height
            ])
        }
    }

    private func handleCaptureWindow(params: [String: AnyCodable]?, requestId: String) {
        guard
            let path = params?["path"]?.string(),
            let windowId = params?["windowId"]?.int()
        else {
            respondError(
                id: requestId,
                code: "INVALID_PARAMS",
                message: "A destination path and a window id are required"
            )
            return
        }

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            let image = CGWindowListCreateImage(
                CGRect.null,
                .optionIncludingWindow,
                CGWindowID(windowId),
                [.bestResolution]
            )

            guard let image = image, self.writePng(image, to: path) else {
                self.respondError(id: requestId, code: "CAPTURE_FAILED", message: "Failed to capture the window")
                return
            }

            self.respond(id: requestId, result: [
                "path": path,
                "width": image.width,
                "height": image.height
            ])
        }
    }

    private func areaRect(params: [String: AnyCodable]?) -> CGRect? {
        guard
            let x = params?["x"]?.double(),
            let y = params?["y"]?.double(),
            let width = params?["width"]?.double(),
            let height = params?["height"]?.double(),
            width > 0,
            height > 0
        else {
            return nil
        }

        return CGRect(x: x, y: y, width: width, height: height)
    }

    private func writePng(_ image: CGImage, to path: String) -> Bool {
        let url = URL(fileURLWithPath: path)
        guard let destination = CGImageDestinationCreateWithURL(
            url as CFURL,
            UTType.png.identifier as CFString,
            1,
            nil
        ) else {
            return false
        }

        CGImageDestinationAddImage(destination, image, nil)
        return CGImageDestinationFinalize(destination)
    }
}
