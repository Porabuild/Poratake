import Cocoa

@main
struct DisplayCoordinatesTest {
    static func main() {
        let primaryHeight: CGFloat = 900
        for (topLeftY, height, expectedCocoaY): (CGFloat, CGFloat, CGFloat) in [
            (0, 52, 848),
            (-200, 52, 1048),
            (1050, 52, -202),
            (-300, 270, 930),
            (1300, 270, -670),
        ] {
            let cocoaY = DisplayCoordinates.cocoaY(
                topLeftY: topLeftY, height: height, primaryHeight: primaryHeight
            )
            precondition(cocoaY == expectedCocoaY)
            precondition(DisplayCoordinates.topLeftY(
                cocoaY: cocoaY, height: height, primaryHeight: primaryHeight
            ) == topLeftY)
        }
        precondition(DisplayCoordinates.cocoaY(topLeftY: -200, height: 52, primaryHeight: 1200) != 1048)
        print("Display coordinate conversions passed")
    }
}
