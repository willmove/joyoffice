import Foundation
import CoreGraphics
guard CommandLine.arguments.count == 3,
      let x = Double(CommandLine.arguments[1]), let y = Double(CommandLine.arguments[2]) else { exit(2) }
let point=CGPoint(x:x,y:y)
CGEvent(mouseEventSource:nil,mouseType:.mouseMoved,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
Thread.sleep(forTimeInterval:0.06)
CGEvent(mouseEventSource:nil,mouseType:.leftMouseDown,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
Thread.sleep(forTimeInterval:0.06)
CGEvent(mouseEventSource:nil,mouseType:.leftMouseUp,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
