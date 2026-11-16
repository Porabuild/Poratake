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
