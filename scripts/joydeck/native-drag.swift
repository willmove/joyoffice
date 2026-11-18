// Send one complete native mouse gesture from the same Quartz event source.
import CoreGraphics
import Darwin

let coordinates = CommandLine.arguments.dropFirst().compactMap(Double.init)
guard coordinates.count == 4, let source = CGEventSource(stateID: .hidSystemState) else { exit(2) }
let start = CGPoint(x: coordinates[0], y: coordinates[1])
let end = CGPoint(x: coordinates[2], y: coordinates[3])

func send(_ type: CGEventType, at point: CGPoint) {
    guard let event = CGEvent(mouseEventSource: source, mouseType: type,
                              mouseCursorPosition: point, mouseButton: .left) else { exit(3) }
    event.setIntegerValueField(.mouseEventClickState, value: 1)
    event.post(tap: .cghidEventTap)
    usleep(120_000)
}

send(.mouseMoved, at: start)
send(.leftMouseDown, at: start)
for i in 1...4 {
    let fraction = CGFloat(i) / 4
    send(.leftMouseDragged, at: CGPoint(x: start.x + (end.x - start.x) * fraction,
                                         y: start.y + (end.y - start.y) * fraction))
}
send(.leftMouseUp, at: end)
