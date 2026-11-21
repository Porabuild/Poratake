import Foundation

struct StreamFailureRecord {
    let error: Error
    let duringStop: Bool
}

final class StreamFailureInbox {
    private let lock = NSLock()
    private var streamID: ObjectIdentifier?
    private var stopping = false
    private var pending: [StreamFailureRecord] = []

    func activate(_ id: ObjectIdentifier) {
        lock.lock()
        defer { lock.unlock() }
        streamID = id
        stopping = false
        pending.removeAll()
    }

    func markStopping() {
        lock.lock()
        defer { lock.unlock() }
        stopping = true
    }

    func isCurrent(_ id: ObjectIdentifier) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return streamID == id
    }

    func enqueue(_ error: Error, from id: ObjectIdentifier) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard streamID == id else { return false }
        pending.append(StreamFailureRecord(error: error, duringStop: stopping))
        return true
    }

    func drain(from id: ObjectIdentifier) -> [StreamFailureRecord] {
        lock.lock()
        defer { lock.unlock() }
        guard streamID == id else { return [] }
        let records = pending
        pending.removeAll()
        return records
    }

    func close() -> [StreamFailureRecord] {
        lock.lock()
        defer { lock.unlock() }
        streamID = nil
        stopping = false
        let records = pending
        pending.removeAll()
        return records
    }
}
