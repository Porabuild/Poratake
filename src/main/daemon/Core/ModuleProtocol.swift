import Foundation

protocol Module {
    var name: String { get }
    func handle(method: String, params: [String: AnyCodable]?, requestId: String)
}

extension Module {
    func respond(id: String, result: Any? = nil) {
        sendResponse(.success(id: id, result: result))
    }
    
    func respondError(id: String, code: String, message: String) {
        sendResponse(.error(id: id, code: code, message: message))
    }
    
    func emit(event: String, data: Any? = nil) {
        sendEvent(Event(event: "\(name):\(event)", data: data))
    }
}
