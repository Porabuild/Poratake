import Vision
import Foundation
import AppKit

class QRCodeModule: Module {
    let name = "qrcode"
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "detect":
            handleDetect(params: params, requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
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
        
        let payload = detectQRCode(from: imagePath)
        respond(id: requestId, result: ["payload": payload])
    }
    
    private func detectQRCode(from imagePath: String) -> String {
        guard let image = NSImage(contentsOfFile: imagePath),
              let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
            return ""
        }
        
        var detectedPayload = ""
        let semaphore = DispatchSemaphore(value: 0)
        
        let request = VNDetectBarcodesRequest { request, error in
            defer { semaphore.signal() }
            
            guard error == nil,
                  let observations = request.results as? [VNBarcodeObservation] else {
                return
            }
            
            for observation in observations {
                if observation.symbology == .qr,
                   let payload = observation.payloadStringValue {
                    detectedPayload = payload
                    return
                }
            }
            
            if let firstObservation = observations.first,
               let payload = firstObservation.payloadStringValue {
                detectedPayload = payload
            }
        }
        
        let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
        
        do {
            try handler.perform([request])
            semaphore.wait()
        } catch {
            return ""
        }
        
        return detectedPayload
    }
}
