import Vision
import Foundation
import ImageIO

class QRCodeModule: Module {
    let name = DaemonContract.QRCode.module
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.QRCode.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .detect:
            handleDetect(params: params, requestId: requestId)
        }
    }
    
    private func handleDetect(params: [String: AnyCodable]?, requestId: String) {
        guard let imagePath = params?["imagePath"]?.string() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Missing imagePath parameter")
            return
        }
        
        guard FileManager.default.fileExists(atPath: imagePath) else {
            respondError(id: requestId, code: "FILE_NOT_FOUND", message: "Image file not found: \(imagePath)")
            return
        }
        
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            let result = Result { try self.detectQRCode(from: imagePath) }
            DispatchQueue.main.async {
                switch result {
                case .success(let payload):
                    self.respond(id: requestId, result: ["payload": payload])
                case .failure(let error):
                    self.respondError(id: requestId, code: "QR_DETECTION_FAILED", message: error.localizedDescription)
                }
            }
        }
    }
    
    private func detectQRCode(from imagePath: String) throws -> String {
        let imageUrl = URL(fileURLWithPath: imagePath) as CFURL
        guard let source = CGImageSourceCreateWithURL(imageUrl, nil),
              let cgImage = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw QRCodeError.imageDecodeFailed
        }
        
        var detectedPayload = ""
        var detectionError: Error?
        let semaphore = DispatchSemaphore(value: 0)
        
        let request = VNDetectBarcodesRequest { request, error in
            defer { semaphore.signal() }
            
            if let error = error {
                detectionError = error
                return
            }

            guard let observations = request.results as? [VNBarcodeObservation] else {
                return
            }
            
            for observation in observations {
                if observation.symbology == .qr,
                   let payload = observation.payloadStringValue {
                    detectedPayload = payload
                    return
                }
            }
        }
        
        let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
        
        try handler.perform([request])
        semaphore.wait()

        if let detectionError = detectionError {
            throw detectionError
        }
        
        return detectedPayload
    }
}

private enum QRCodeError: LocalizedError {
    case imageDecodeFailed

    var errorDescription: String? {
        "Failed to decode image"
    }
}
