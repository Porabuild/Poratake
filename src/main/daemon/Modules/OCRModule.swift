import Vision
import Foundation
import ImageIO

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
        
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            let result = Result { try self.recognizeText(from: imagePath) }
            DispatchQueue.main.async {
                switch result {
                case .success(let text):
                    self.respond(id: requestId, result: ["text": text])
                case .failure(let error):
                    self.respondError(id: requestId, code: "OCR_FAILED", message: error.localizedDescription)
                }
            }
        }
    }
    
    private func recognizeText(from imagePath: String) throws -> String {
        let imageUrl = URL(fileURLWithPath: imagePath) as CFURL
        guard let source = CGImageSourceCreateWithURL(imageUrl, nil),
              let cgImage = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw OCRError.imageDecodeFailed
        }

        let mixedLanguageText = try performRecognition(cgImage: cgImage) { request in
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = false
            request.revision = VNRecognizeTextRequestRevision3
            let languages = recognitionLanguages(for: request)
            if !languages.isEmpty {
                request.recognitionLanguages = languages
            }
        }
        if !mixedLanguageText.isEmpty {
            return mixedLanguageText
        }

        return try performRecognition(cgImage: cgImage) { request in
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = true
            request.revision = VNRecognizeTextRequestRevision3
            request.automaticallyDetectsLanguage = true
        }
    }

    private func performRecognition(
        cgImage: CGImage,
        configure: (VNRecognizeTextRequest) -> Void
    ) throws -> String {
        var recognizedText = ""
        var recognitionError: Error?
        let semaphore = DispatchSemaphore(value: 0)

        let request = VNRecognizeTextRequest { request, error in
            defer { semaphore.signal() }

            if let error = error {
                recognitionError = error
                return
            }

            guard let observations = request.results as? [VNRecognizedTextObservation] else {
                return
            }

            let texts = observations.compactMap { observation in
                self.bestCandidateString(from: observation)
            }

            recognizedText = texts.joined(separator: "\n")
        }

        configure(request)

        let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
        try handler.perform([request])
        semaphore.wait()

        if let recognitionError = recognitionError {
            throw recognitionError
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
        return preferredLanguages.filter { supportedLanguages.contains($0) }
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

private enum OCRError: LocalizedError {
    case imageDecodeFailed

    var errorDescription: String? {
        "Failed to decode image"
    }
}
