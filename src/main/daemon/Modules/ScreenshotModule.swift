import Cocoa
import Foundation
import ImageIO
import UniformTypeIdentifiers

class ScreenshotModule: Module {
    let name = DaemonContract.Screenshot.module

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.Screenshot.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .captureArea:
            handleCaptureArea(params: params, requestId: requestId)
        case .captureWindow:
            handleCaptureWindow(params: params, requestId: requestId)
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
        if let value = params?["excludedWindowIds"], !(value.value is NSNull), value.array() == nil {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Invalid excluded window ids")
            return
        }
        let rawExcluded = params?["excludedWindowIds"]?.array() ?? []
        guard rawExcluded.allSatisfy({ value in
            guard let number = value as? Int, let id = CGWindowID(exactly: number) else { return false }
            return id != kCGNullWindowID
        }) else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Invalid excluded window ids")
            return
        }
        let excludedWindowIDs = Set(rawExcluded.compactMap { ($0 as? Int).flatMap(CGWindowID.init(exactly:)) })
        var displayID: CGDirectDisplayID?
        if let value = params?["displayId"], !(value.value is NSNull) {
            guard let number = value.value as? Int, let id = CGDirectDisplayID(exactly: number) else {
                respondError(id: requestId, code: "INVALID_PARAMS", message: "Invalid display id")
                return
            }
            displayID = id
        }
#if WINDOW_ID_VALIDATION_TEST
        respond(id: requestId, result: ["validated": true])
        return
#endif

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            if cached, let image = FrozenFrameStore.crop(to: rect) {
                self.respondCapture(image, path: path, requestId: requestId)
                return
            }
            Task { @MainActor in
                do {
                    let image: CGImage? = try await coordinateAreaCapture(
                        registry: RecordingCaptureWindowRegistry.shared,
                        excludedWindowIDs: excludedWindowIDs,
                        unfiltered: {
                            await withCheckedContinuation { continuation in
                                DispatchQueue.global(qos: .userInitiated).async {
                                    let image = CGWindowListCreateImage(rect, .optionOnScreenOnly, kCGNullWindowID, [.bestResolution])
                                    continuation.resume(returning: image)
                                }
                            }
                        },
                        filtered: { ids in
                            let displayBounds = NSScreen.screens.compactMap { screen -> CGRect? in
                                guard let number = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber else { return nil }
                                return CGDisplayBounds(number.uint32Value)
                            }
                            if screenshotNeedsWindowListCapture(rect: rect, displayBounds: displayBounds) {
                                return try await self.captureAreaUsingWindowList(rect, excluding: ids)
                            }
                            guard #available(macOS 14.0, *) else {
                                throw NSError(domain: "Screenshot", code: 5, userInfo: [NSLocalizedDescriptionKey: "Cannot exclude capture controls on this macOS version"])
                            }
                            return try await captureAreaExcludingWindows(rect, displayID: displayID, excludedWindowIDs: ids)
                        }
                    )
                    DispatchQueue.global(qos: .userInitiated).async {
                        self.respondCapture(image, path: path, requestId: requestId)
                    }
                } catch {
                    self.respondError(id: requestId, code: "CAPTURE_FAILED", message: error.localizedDescription)
                }
            }
        }
    }

    private func captureAreaUsingWindowList(_ rect: CGRect, excluding excludedWindowIDs: Set<CGWindowID>) async throws -> CGImage {
        let result: Result<CGImage, Error> = await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                guard let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]],
                      let ids = screenshotWindowIDs(from: windows, excluding: excludedWindowIDs) else {
                    continuation.resume(returning: .failure(NSError(domain: "Screenshot", code: 6, userInfo: [NSLocalizedDescriptionKey: "Could not verify the capture window list"])))
                    return
                }
                let numbers = ids.map { NSNumber(value: Int32(bitPattern: $0)) } as CFArray
                guard let image = CGImage(windowListFromArrayScreenBounds: rect, windowArray: numbers, imageOption: [.bestResolution]) else {
                    continuation.resume(returning: .failure(NSError(domain: "Screenshot", code: 7, userInfo: [NSLocalizedDescriptionKey: "Failed to capture the area"])))
                    return
                }
                continuation.resume(returning: .success(image))
            }
        }
        return try result.get()
    }

    private func respondCapture(_ image: CGImage?, path: String, requestId: String) {
        guard let image = image, writePng(image, to: path) else {
            respondError(id: requestId, code: "CAPTURE_FAILED", message: "Failed to capture the area")
            return
        }
        respond(id: requestId, result: ["path": path, "width": image.width, "height": image.height])
    }

    private func handleCaptureWindow(params: [String: AnyCodable]?, requestId: String) {
        guard
            let path = params?["path"]?.string(),
            let rawWindowId = params?["windowId"]?.value as? Int,
            let windowId = CGWindowID(exactly: rawWindowId),
            windowId != kCGNullWindowID
        else {
            respondError(
                id: requestId,
                code: "INVALID_PARAMS",
                message: "A destination path and a window id are required"
            )
            return
        }
#if WINDOW_ID_VALIDATION_TEST
        respond(id: requestId, result: ["validated": true])
        return
#endif

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }

            let image = CGWindowListCreateImage(
                CGRect.null,
                .optionIncludingWindow,
                windowId,
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
