import Foundation

class Router {
    private var modules: [String: Module] = [:]
    
    func register(_ module: Module) {
        modules[module.name] = module
    }
    
    func route(_ request: Request) {
        if request.module == "system" {
            handleSystem(request)
            return
        }
        
        guard let module = modules[request.module] else {
            sendResponse(.error(
                id: request.id,
                code: "MODULE_NOT_FOUND",
                message: "Module '\(request.module)' not found"
            ))
            return
        }
        
        module.handle(
            method: request.method,
            params: request.params,
            requestId: request.id
        )
    }
    
    private func handleSystem(_ request: Request) {
        switch request.method {
        case "ping":
            sendResponse(.success(id: request.id, result: ["pong": true]))
        case "list-modules":
            let moduleNames = Array(modules.keys).sorted()
            sendResponse(.success(id: request.id, result: ["modules": moduleNames]))
        case "quit":
            sendResponse(.success(id: request.id, result: ["status": "exiting"]))
            exit(0)
        default:
            sendResponse(.error(
                id: request.id,
                code: "METHOD_NOT_FOUND",
                message: "System method '\(request.method)' not found"
            ))
        }
    }
}
