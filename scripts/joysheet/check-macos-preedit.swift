// Check the native screenshot of A1 while the system Pinyin candidate panel is open.
import Foundation
import Vision

guard CommandLine.arguments.count == 2 else { exit(2) }
let image = URL(fileURLWithPath: CommandLine.arguments[1])
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = false
request.recognitionLanguages = ["en-US", "zh-Hans"]
try VNImageRequestHandler(url: image).perform([request])
let lines = request.results?.compactMap { $0.topCandidates(1).first?.string } ?? []
guard lines.contains(where: { $0.lowercased().filter { !$0.isWhitespace }.contains("zhongwen") }) else {
    FileHandle.standardError.write(Data("Full Pinyin spelling is not visible on a single line in the captured cell editor.\n".utf8))
    exit(1)
}
