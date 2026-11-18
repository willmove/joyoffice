// WindowServer presentation timestamp and pixel fingerprint of a crop from one application window.
import AppKit
import Foundation
import ScreenCaptureKit
import CoreMedia
import CoreVideo
import Darwin

final class Samples: NSObject, SCStreamOutput {
    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, let meta = (CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]])?.first,
              (meta[.status] as? Int) == SCFrameStatus.complete.rawValue,
              let displayed = meta[.displayTime] as? UInt64, let buffer = sample.imageBuffer else { return }
        CVPixelBufferLockBaseAddress(buffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(buffer, .readOnly) }
        guard let data = CVPixelBufferGetBaseAddress(buffer) else { return }
        let stride = CVPixelBufferGetBytesPerRow(buffer)
        let cols = CVPixelBufferGetWidth(buffer)*4
        var hash: UInt64 = 14695981039346656037
        for row in 0..<CVPixelBufferGetHeight(buffer) {
            let p = data.advanced(by: row*stride).assumingMemoryBound(to: UInt8.self)
            for j in 0..<cols { hash = (hash ^ UInt64(p[j])) &* 1099511628211 }
        }
        var timebase = mach_timebase_info_data_t()
        mach_timebase_info(&timebase)
        // Divide first to avoid overflowing at long uptimes.
        let (numer, denom) = (UInt64(timebase.numer), UInt64(timebase.denom))
        let stamp = displayed / denom * numer + displayed % denom * numer / denom
        print("\(stamp) \(hash)")
        fflush(stdout)
    }
}

@main struct Capture {
    @MainActor static func main() async throws {
        _ = NSApplication.shared
        let args = CommandLine.arguments
        guard args.count == 6, let pid = Int32(args[1]), let x = Int(args[2]), let y = Int(args[3]),
              let width = Int(args[4]), let height = Int(args[5]) else { exit(2) }
        let sources = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        guard let window = sources.windows.first(where: { $0.owningApplication?.processID == pid && $0.windowLayer == 0 }) else { exit(3) }
        let filter = SCContentFilter(desktopIndependentWindow: window)
        let cfg = SCStreamConfiguration()
        cfg.width = width
        cfg.height = height
        cfg.sourceRect = CGRect(x: x, y: y, width: width, height: height)
        cfg.showsCursor = false
        cfg.ignoreShadowsSingleWindow = true
        cfg.minimumFrameInterval = CMTime(value: 1, timescale: 120)
        cfg.pixelFormat = kCVPixelFormatType_32BGRA
        let observer = Samples()
        let stream = SCStream(filter: filter, configuration: cfg, delegate: nil)
        try stream.addStreamOutput(observer, type: .screen, sampleHandlerQueue: DispatchQueue(label: "joyoffice.frames"))
        try await stream.startCapture()
        try await Task.sleep(nanoseconds: 300_000_000_000)
        try await stream.stopCapture()
    }
}
