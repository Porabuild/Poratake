import CoreGraphics

func recordingDisplayID(
    available: [CGDirectDisplayID],
    windowDisplayID: CGDirectDisplayID?,
    requestedDisplayID: CGDirectDisplayID?,
    mainDisplayID: CGDirectDisplayID
) throws -> CGDirectDisplayID {
    if let selected = windowDisplayID ?? requestedDisplayID {
        guard available.contains(selected) else {
            throw RecorderError.configuration("The selected display is no longer available")
        }
        return selected
    }
    guard let display = available.first(where: { $0 == mainDisplayID }) ?? available.first else {
        throw RecorderError.configuration("No display found")
    }
    return display
}

func recordingSourceRect(selection: CGRect, display: CGRect) throws -> CGRect {
    guard [selection.minX, selection.minY, selection.width, selection.height,
           display.minX, display.minY, display.width, display.height].allSatisfy(\.isFinite),
          selection.width > 0, selection.height > 0,
          display.width > 0, display.height > 0,
          display.contains(selection) else {
        throw RecorderError.configuration("The selected recording area is no longer within its display")
    }
    return CGRect(
        x: selection.origin.x - display.origin.x,
        y: selection.origin.y - display.origin.y,
        width: selection.width,
        height: selection.height
    )
}
