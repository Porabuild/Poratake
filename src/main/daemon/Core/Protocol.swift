import Foundation

struct Request: Codable {
    let id: String
    let module: String
    let method: String
    let params: [String: AnyCodable]?
}

struct Response: Codable {
    let id: String
    let success: Bool
    let result: AnyCodable?
    let error: ResponseError?
    
    static func success(id: String, result: Any? = nil) -> Response {
        Response(
            id: id,
            success: true,
            result: result.map { AnyCodable($0) },
            error: nil
        )
    }
    
    static func error(id: String, code: String, message: String) -> Response {
        Response(
            id: id,
            success: false,
            result: nil,
            error: ResponseError(code: code, message: message)
        )
    }
}

struct ResponseError: Codable {
    let code: String
    let message: String
}

struct Event: Codable {
    let event: String
    let data: AnyCodable?
    
    init(event: String, data: Any? = nil) {
        self.event = event
        self.data = data.map { AnyCodable($0) }
    }
}

struct AnyCodable: Codable {
    let value: Any
    
    init(_ value: Any) {
        self.value = value
    }
    
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        
        if container.decodeNil() {
            value = NSNull()
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let string = try? container.decode(String.self) {
            value = string
        } else if let array = try? container.decode([AnyCodable].self) {
            value = array.map { $0.value }
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues { $0.value }
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Unsupported type"
            )
        }
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        
        switch value {
        case is NSNull:
            try container.encodeNil()
        case let bool as Bool:
            try container.encode(bool)
        case let int as Int:
            try container.encode(int)
        case let double as Double:
            try container.encode(double)
        case let string as String:
            try container.encode(string)
        case let cgFloat as CGFloat:
            try container.encode(Double(cgFloat))
        case let float as Float:
            try container.encode(Double(float))
        case let array as [Any]:
            try container.encode(array.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            try container.encodeNil()
        }
    }
    
    func string() -> String? {
        value as? String
    }
    
    func int() -> Int? {
        if let i = value as? Int { return i }
        if let d = value as? Double { return Int(d) }
        return nil
    }
    
    func double() -> Double? {
        if let d = value as? Double { return d }
        if let i = value as? Int { return Double(i) }
        return nil
    }
    
    func bool() -> Bool? {
        value as? Bool
    }
    
    func array() -> [Any]? {
        value as? [Any]
    }
    
    func dict() -> [String: Any]? {
        value as? [String: Any]
    }
}

func sendResponse(_ response: Response) {
    guard let data = try? JSONEncoder().encode(response),
          let json = String(data: data, encoding: .utf8) else {
        return
    }
    print(json)
    fflush(stdout)
}

func sendEvent(_ event: Event) {
    guard let data = try? JSONEncoder().encode(event),
          let json = String(data: data, encoding: .utf8) else {
        return
    }
    print(json)
    fflush(stdout)
}

func parseRequest(_ line: String) -> Request? {
    guard let data = line.data(using: .utf8) else { return nil }
    return try? JSONDecoder().decode(Request.self, from: data)
}
