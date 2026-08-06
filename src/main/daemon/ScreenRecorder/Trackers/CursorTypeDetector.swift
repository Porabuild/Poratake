import AppKit
import Foundation

@_silgen_name("CGSMainConnectionID")
func CGSMainConnectionID() -> Int32

@_silgen_name("CGSGetGlobalCursorDataSize")
func CGSGetGlobalCursorDataSize(_ connection: Int32, _ size: UnsafeMutablePointer<Int32>) -> Int32

@_silgen_name("CGSGetGlobalCursorData")
func CGSGetGlobalCursorData(
    _ connection: Int32,
    _ data: UnsafeMutableRawPointer,
    _ size: UnsafeMutablePointer<Int32>,
    _ rowBytes: UnsafeMutablePointer<Int32>,
    _ dimensions: UnsafeMutablePointer<CGRect>,
    _ hotSpot: UnsafeMutablePointer<CGPoint>,
    _ depth: UnsafeMutablePointer<Int32>,
    _ components: UnsafeMutablePointer<Int32>,
    _ bitsPerComponent: UnsafeMutablePointer<Int32>
) -> Int32

enum CursorType: String, CaseIterable {
    case arrow
    case pointingHand
    case openHand
    case closedHand
    case iBeam
    case crosshair
    case resizeLeftRight
    case resizeUpDown
}

class CursorTypeDetector {
    private(set) var currentType: CursorType = .arrow
    private var lastSignature: CursorSignature? = nil
    private var learnedHashes: [Int: CursorType] = [:]
    
    private struct CursorSignature: Equatable {
        let width: Int
        let height: Int
        let hotSpotX: Int
        let hotSpotY: Int
        let dataHash: Int
    }
    
    func checkForChange() -> Bool {
        guard let currentSignature = getCurrentCursorSignature() else {
            return false
        }
        
        if let last = lastSignature, last == currentSignature {
            return false
        }
        
        lastSignature = currentSignature
        let newType = detectType(signature: currentSignature)
        
        if newType != currentType {
            currentType = newType
            return true
        }
        
        return false
    }
    
    private func getCurrentCursorSignature() -> CursorSignature? {
        let connection = CGSMainConnectionID()
        var dataSize: Int32 = 0
        
        guard CGSGetGlobalCursorDataSize(connection, &dataSize) == 0, dataSize > 0 else {
            return nil
        }
        
        let data = UnsafeMutableRawPointer.allocate(byteCount: Int(dataSize), alignment: 1)
        defer { data.deallocate() }
        
        var size: Int32 = dataSize
        var rowBytes: Int32 = 0
        var dimensions = CGRect.zero
        var hotSpot = CGPoint.zero
        var depth: Int32 = 0
        var components: Int32 = 0
        var bitsPerComponent: Int32 = 0
        
        guard CGSGetGlobalCursorData(
            connection,
            data,
            &size,
            &rowBytes,
            &dimensions,
            &hotSpot,
            &depth,
            &components,
            &bitsPerComponent
        ) == 0 else {
            return nil
        }
        
        let dataBuffer = Data(bytes: data, count: Int(size))
        
        return CursorSignature(
            width: Int(dimensions.width),
            height: Int(dimensions.height),
            hotSpotX: Int(hotSpot.x),
            hotSpotY: Int(hotSpot.y),
            dataHash: dataBuffer.hashValue
        )
    }
    
    private func detectType(signature: CursorSignature) -> CursorType {
        let w = signature.width
        let h = signature.height
        let hx = signature.hotSpotX
        let hy = signature.hotSpotY
        
        if w == 17 && h == 23 && hx == 4 && hy == 4 {
            return .arrow
        }
        
        if w == 9 && h == 18 && hx == 4 && hy == 9 {
            return .iBeam
        }
        
        if w == 32 && h == 32 && hx == 13 && hy == 8 {
            return .pointingHand
        }
        
        if w == 32 && h == 32 && hx == 16 && hy == 16 {
            if let knownType = learnedHashes[signature.dataHash] {
                return knownType
            }
            return .openHand
        }
        
        if w == 24 && h == 24 && hx == 11 && hy == 11 {
            return .crosshair
        }
        
        if w == 24 && h == 24 && hx == 12 && hy == 12 {
            if let knownType = learnedHashes[signature.dataHash] {
                return knownType
            }
            return .resizeLeftRight
        }
        
        if w == 34 && h == 46 && hx == 8 && hy == 8 {
            return .arrow
        }
        
        if w == 18 && h == 36 && hx == 8 && hy == 18 {
            return .iBeam
        }
        
        if w == 64 && h == 64 && hx == 26 && hy == 16 {
            return .pointingHand
        }
        
        if w == 64 && h == 64 && hx == 32 && hy == 32 {
            if let knownType = learnedHashes[signature.dataHash] {
                return knownType
            }
            return .openHand
        }
        
        if w == 48 && h == 48 && hx == 22 && hy == 22 {
            return .crosshair
        }
        
        if w == 48 && h == 48 && hx == 24 && hy == 24 {
            if let knownType = learnedHashes[signature.dataHash] {
                return knownType
            }
            return .resizeLeftRight
        }
        
        return .arrow
    }
}
