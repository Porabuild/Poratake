import ApplicationServices
import Foundation

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("\(message)\n".utf8))
    exit(1)
}

guard CommandLine.arguments.count == 2, let pid = pid_t(CommandLine.arguments[1]), pid > 0 else {
    fail("Provide the isolated Save probe's process ID")
}
guard AXIsProcessTrusted() else {
    fail("Save confirmation requires existing macOS Accessibility authorization")
}

func value(_ element: AXUIElement, _ attribute: String) -> CFTypeRef? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute as CFString, &result) == .success else {
        return nil
    }
    return result
}

func saveButtons(in element: AXUIElement, insideSheet: Bool, depth: Int) -> [AXUIElement] {
    guard depth < 14 else { return [] }
    let role = value(element, kAXRoleAttribute) as? String
    let insideSheet = insideSheet || (role == kAXSheetRole
        && value(element, kAXIdentifierAttribute) as? String == "save-panel")
    if insideSheet && role == kAXButtonRole
        && value(element, kAXIdentifierAttribute) as? String == "OKButton"
        && value(element, kAXEnabledAttribute) as? Bool == true {
        return [element]
    }
    let children = value(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
    return children.flatMap { saveButtons(in: $0, insideSheet: insideSheet, depth: depth + 1) }
}

let application = AXUIElementCreateApplication(pid)
_ = AXUIElementSetMessagingTimeout(application, 2)
let windows = value(application, kAXWindowsAttribute) as? [AXUIElement] ?? []
let buttons = windows.flatMap { saveButtons(in: $0, insideSheet: false, depth: 0) }
guard buttons.count == 1 else {
    fail("Expected one enabled Save button in a sheet of the isolated editor, found \(buttons.count)")
}
let result = AXUIElementPerformAction(buttons[0], kAXPressAction as CFString)
guard result == .success else {
    fail("The Save button returned Accessibility error \(result.rawValue)")
}
