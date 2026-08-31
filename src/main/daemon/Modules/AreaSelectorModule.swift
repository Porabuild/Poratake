import Foundation

class AreaSelectorModule: Module {
    let name = DaemonContract.AreaSelector.module

    func handle(method: String, params: [String: AnyCodable]?, requestId: String) {
        guard let method = DaemonContract.AreaSelector.Method(rawValue: method) else {
            respondError(id: requestId, code: "METHOD_NOT_FOUND", message: "Unknown method: \(method)")
            return
        }

        switch method {
        case .disableWindowTransitions,
             .hideWindowWithoutTransitions,
             .showWindowWithoutTransitions,
             .setWindowRegion:
            respond(id: requestId, result: ["disabled": false])
        case .getForegroundWindow:
            respond(id: requestId, result: ["windowHandle": 0])
        case .setForegroundWindow:
            respond(id: requestId, result: ["restored": false])
        }
    }
}
