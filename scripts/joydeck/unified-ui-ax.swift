// Native acceptance helper. Only the explicitly supplied application PID is inspected.
import Cocoa
import ApplicationServices
let args = CommandLine.arguments
guard args.count >= 3, let value = Int32(args[1]) else { exit(2) }
let app = AXUIElementCreateApplication(value)
func attribute(_ e: AXUIElement, _ key: String) -> CFTypeRef? {
    var result: CFTypeRef?
    AXUIElementCopyAttributeValue(e, key as CFString, &result)
    return result
}
func children(_ e: AXUIElement) -> [AXUIElement] {
    return attribute(e, kAXChildrenAttribute) as? [AXUIElement] ?? []
}
func find(_ e: AXUIElement, _ name: String, _ depth: Int = 0) -> AXUIElement? {
    if depth > 16 { return nil }
    for key in [kAXIdentifierAttribute, kAXTitleAttribute, kAXDescriptionAttribute] {
        if attribute(e, key) as? String == name { return e }
    }
    for child in children(e) { if let found = find(child, name, depth + 1) { return found } }
    return nil
}
func node(_ e: AXUIElement, _ depth: Int = 0) -> [String: Any] {
    var result: [String: Any] = [:]
    for (key, name) in [(kAXRoleAttribute, "role"), (kAXTitleAttribute, "title"), (kAXDescriptionAttribute, "label"), (kAXIdentifierAttribute, "id"), (kAXValueAttribute, "value"), (kAXEnabledAttribute, "enabled"), (kAXFocusedAttribute, "focused")] {
        if let v = attribute(e, key) { result[name] = String(describing: v) }
    }
    if depth < 16 { result["children"] = children(e).prefix(300).map { node($0, depth + 1) } }
    return result
}
AXUIElementSetAttributeValue(app, "AXEnhancedUserInterface" as CFString, kCFBooleanTrue)
var output: Any = [:]
switch args[2] {
case "tree":
    output = (attribute(app, kAXWindowsAttribute) as? [AXUIElement] ?? []).map { node($0) }
case "press", "set", "focus":
    guard args.count >= 4, let target = find(app, args[3]) else { fputs("Control missing\n", stderr); exit(3) }
    let result: AXError
    if args[2] == "press" { result = AXUIElementPerformAction(target, kAXPressAction as CFString) }
    else if args[2] == "focus" { result = AXUIElementSetAttributeValue(target, kAXFocusedAttribute as CFString, kCFBooleanTrue) }
    else { guard args.count >= 5 else { exit(2) }; result = AXUIElementSetAttributeValue(target, kAXValueAttribute as CFString, args[4] as CFString) }
    guard result == .success else { fputs("AX action failed: \(result.rawValue)\n", stderr); exit(4) }
    output = ["ok": true]
default: exit(2)
}
let data = try JSONSerialization.data(withJSONObject: output, options: [.sortedKeys])
print(String(data: data, encoding: .utf8)!)
