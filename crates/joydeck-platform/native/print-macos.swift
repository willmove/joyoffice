import AppKit
import PDFKit
import Foundation
let app=NSApplication.shared
app.setActivationPolicy(.regular)
app.finishLaunching()
guard CommandLine.arguments.count==2,let pdf=PDFDocument(url:URL(fileURLWithPath:CommandLine.arguments[1])),let first=pdf.page(at:0)else{exit(2)}
let info=NSPrintInfo()
let bounds=first.bounds(for:.mediaBox)
info.orientation=bounds.width>bounds.height ? .landscape : .portrait
guard let operation=pdf.printOperation(for:info,scalingMode:.pageScaleToFit,autoRotate:true)else{exit(2)}
operation.showsPrintPanel=true
operation.showsProgressPanel=true
operation.printPanel.options.formUnion([.showsPaperSize,.showsOrientation,.showsScaling,.showsPageRange,.showsPreview])
app.activate(ignoringOtherApps:true)
let result=operation.run()
print(result ? "completed" : "cancelled")
