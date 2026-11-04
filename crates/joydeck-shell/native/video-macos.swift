// Static slideshow encoder for the JoyDeck-owned snapshot manifest.
import AVFoundation
import CoreGraphics
import CoreVideo
import Foundation
import ImageIO

private struct VideoError: LocalizedError {
    let reason: String
    var errorDescription: String? { reason }

    init(_ reason: String) { self.reason = reason }
}

private struct Manifest: Decodable {
    let pages: [String]
    let seconds: Double
    let fps: Int
}

private struct Page {
    let source: CGImageSource
    let width: Int
    let height: Int
}

private func checkFile(_ url: URL, limit: Int) throws {
    let info = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
    guard info.isRegularFile == true, info.isSymbolicLink == false,
          let size = info.fileSize, size > 0, size <= limit else {
        throw VideoError("Snapshot file is not a bounded regular file")
    }
}

private func openPage(_ name: String, in folder: URL) throws -> Page {
    guard !name.contains("/"), !name.contains("\\"), name.lowercased().hasSuffix(".png") else {
        throw VideoError("Invalid local page name")
    }
    let url = folder.appendingPathComponent(name, isDirectory: false)
    try checkFile(url, limit: 64 << 20)
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          let type = CGImageSourceGetType(source), type as String == "public.png",
          let metadata = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [String: Any],
          let width = metadata[kCGImagePropertyPixelWidth as String] as? Int,
          let height = metadata[kCGImagePropertyPixelHeight as String] as? Int,
          (1...8192).contains(width), (1...8192).contains(height), width * height <= 16_000_000 else {
        throw VideoError("Invalid PNG or page dimensions exceed budget")
    }
    return Page(source: source, width: width, height: height)
}

private func render(_ page: Page, into pool: CVPixelBufferPool, width: Int, height: Int) throws -> CVPixelBuffer {
    guard let image = CGImageSourceCreateImageAtIndex(page.source, 0, nil) else {
        throw VideoError("Cannot decode snapshot PNG")
    }
    var allocated: CVPixelBuffer?
    guard CVPixelBufferPoolCreatePixelBuffer(nil, pool, &allocated) == kCVReturnSuccess,
          let buffer = allocated, CVPixelBufferLockBaseAddress(buffer, []) == kCVReturnSuccess else {
        throw VideoError("Cannot allocate video frame")
    }
    defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
    guard let bytes = CVPixelBufferGetBaseAddress(buffer),
          let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
          let context = CGContext(
            data: bytes, width: width, height: height, bitsPerComponent: 8,
            bytesPerRow: CVPixelBufferGetBytesPerRow(buffer), space: colorSpace,
            bitmapInfo: CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue
          ) else {
        throw VideoError("Cannot create the video frame drawing context")
    }
    let scale = min(Double(width) / Double(image.width), Double(height) / Double(image.height))
    let imageWidth = Double(image.width) * scale
    let imageHeight = Double(image.height) * scale
    context.setFillColor(CGColor(gray: 1, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: width, height: height))
    context.interpolationQuality = .high
    context.draw(image, in: CGRect(x: (Double(width) - imageWidth) / 2,
                                   y: (Double(height) - imageHeight) / 2,
                                   width: imageWidth, height: imageHeight))
    return buffer
}

private func encode(_ manifestPath: String) throws {
    let url = URL(fileURLWithPath: manifestPath).standardizedFileURL
    try checkFile(url, limit: 1 << 20)
    let manifest = try JSONDecoder().decode(Manifest.self, from: Data(contentsOf: url))
    let duration = manifest.seconds * Double(manifest.pages.count)
    guard let firstName = manifest.pages.first, manifest.pages.count <= 120,
          manifest.seconds.isFinite, (0.1...30.0).contains(manifest.seconds),
          duration <= 60, (1...30).contains(manifest.fps) else {
        throw VideoError("Video duration, page count or frame rate exceeds budget")
    }

    let folder = url.deletingLastPathComponent()
    let first = try openPage(firstName, in: folder)
    let scale = min(1, 1280.0 / Double(first.width), 720.0 / Double(first.height))
    let width = max(2, Int(Double(first.width) * scale) / 2 * 2)
    let height = max(2, Int(Double(first.height) * scale) / 2 * 2)
    let destination = folder.appendingPathComponent("video.mp4")
    let writer = try AVAssetWriter(outputURL: destination, fileType: .mp4)
    let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
        AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: width, AVVideoHeightKey: height,
        AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: 4_000_000],
    ])
    let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
        kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        kCVPixelBufferWidthKey as String: width, kCVPixelBufferHeightKey as String: height,
        kCVPixelBufferCGImageCompatibilityKey as String: true,
        kCVPixelBufferCGBitmapContextCompatibilityKey as String: true,
    ])
    guard writer.canAdd(input) else { throw VideoError("Unsupported H.264 settings") }
    writer.add(input)
    guard writer.startWriting() else { throw writer.error ?? VideoError("Cannot start encoder") }
    defer {
        if writer.status == .writing { writer.cancelWriting() }
    }
    writer.startSession(atSourceTime: .zero)
    guard let pool = adaptor.pixelBufferPool else { throw VideoError("Video frame pool unavailable") }

    let framesPerSlide = max(1, Int((manifest.seconds * Double(manifest.fps)).rounded()))
    for (index, name) in manifest.pages.enumerated() {
        try autoreleasepool {
            let page = index == 0 ? first : try openPage(name, in: folder)
            let frame = try render(page, into: pool, width: width, height: height)
            for sample in 0..<framesPerSlide {
                while !input.isReadyForMoreMediaData {
                    guard writer.status == .writing else {
                        throw writer.error ?? VideoError("Video encoder stopped")
                    }
                    Thread.sleep(forTimeInterval: 0.002)
                }
                let offset = manifest.seconds * (Double(index) + Double(sample) / Double(framesPerSlide))
                let timestamp = CMTime(seconds: offset, preferredTimescale: 60_000)
                guard adaptor.append(frame, withPresentationTime: timestamp) else {
                    throw writer.error ?? VideoError("Cannot encode snapshot frame")
                }
            }
        }
    }
    writer.endSession(atSourceTime: CMTime(seconds: duration, preferredTimescale: 60_000))
    input.markAsFinished()
    let finished = DispatchSemaphore(value: 0)
    writer.finishWriting { finished.signal() }
    finished.wait()
    guard writer.status == .completed else { throw writer.error ?? VideoError("Cannot finish video") }
}

do {
    guard CommandLine.arguments.count == 2 else { throw VideoError("Expected local video manifest") }
    try encode(CommandLine.arguments[1])
} catch {
    FileHandle.standardError.write(Data((error.localizedDescription + "\n").utf8))
    exit(1)
}
