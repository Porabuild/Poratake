import Vision
import Foundation
import AppKit

class OCRModule: Module {
    let name = "ocr"
    
    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        switch method {
        case "recognize":
            handleRecognize(params: params, requestId: requestId)
        default:
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
        }
    }
    
    private func handleRecognize(params: [String: AnyCodable]?, requestId: String) {
        guard let imagePath = params?["imagePath"]?.string() else {
            respondError(id: requestId, code: "INVALID_PARAMS", message: "Missing imagePath parameter")
            return
        }
        
        guard FileManager.default.fileExists(atPath: imagePath) else {
            respondError(id: requestId, code: "FILE_NOT_FOUND", message: "Image file not found: \(imagePath)")
            return
        }
        
        let text = recognizeText(from: imagePath)
        respond(id: requestId, result: ["text": text])
    }
    
    private func recognizeText(from imagePath: String) -> String {
        guard let image = NSImage(contentsOfFile: imagePath),
              let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
            return ""
        }

        let mixedLanguageText = performRecognition(cgImage: cgImage) { request in
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = false
            request.revision = VNRecognizeTextRequestRevision3
            request.recognitionLanguages = recognitionLanguages(for: request)
        }
        if !mixedLanguageText.isEmpty {
            return mixedLanguageText
        }

        return performRecognition(cgImage: cgImage) { request in
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = true
            request.revision = VNRecognizeTextRequestRevision3
            request.automaticallyDetectsLanguage = true
        }
    }

    private func performRecognition(
        cgImage: CGImage,
        configure: (VNRecognizeTextRequest) -> Void
    ) -> String {
        var recognizedText = ""
        let semaphore = DispatchSemaphore(value: 0)

        let request = VNRecognizeTextRequest { request, error in
            defer { semaphore.signal() }

            guard error == nil,
                  let observations = request.results as? [VNRecognizedTextObservation] else {
                return
            }

            let texts = observations.compactMap { observation in
                self.bestCandidateString(from: observation)
            }

            recognizedText = texts.joined(separator: "\n")
        }

        configure(request)

        let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
        do {
            try handler.perform([request])
            semaphore.wait()
        } catch {
            return ""
        }

        return recognizedText.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func recognitionLanguages(for request: VNRecognizeTextRequest) -> [String] {
        let preferredLanguages = [
            "zh-Hans", "zh-Hant", "zh-HK", "zh-TW",
            "en-US", "en-GB",
            "es-ES", "es-MX",
            "hi-IN",
            "ar-SA", "ar",
            "pt-BR", "pt-PT",
            "ru-RU",
            "ja-JP",
            "de-DE",
            "fr-FR"
        ]
        let supportedLanguages = Set((try? request.supportedRecognitionLanguages()) ?? [])
        let selectedLanguages = preferredLanguages.filter { supportedLanguages.contains($0) }

        if !selectedLanguages.isEmpty {
            return selectedLanguages
        }

        if supportedLanguages.contains("en-US") {
            return ["en-US"]
        }

        return ["en-US", "zh-Hans", "zh-Hant"]
    }

    private func bestCandidateString(from observation: VNRecognizedTextObservation) -> String? {
        let candidates = observation.topCandidates(5)
        if let cjkCandidate = candidates.first(where: { containsCJK($0.string) }) {
            return cjkCandidate.string
        }

        return candidates.first?.string
    }

    private func containsCJK(_ text: String) -> Bool {
        text.unicodeScalars.contains { scalar in
            let value = scalar.value
            return (value >= 0x3400 && value <= 0x4DBF) ||
                (value >= 0x4E00 && value <= 0x9FFF) ||
                (value >= 0xF900 && value <= 0xFAFF)
        }
    }
}
