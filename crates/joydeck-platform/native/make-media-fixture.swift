import Foundation
import AVFoundation
import CoreVideo
let path=CommandLine.arguments[1]
let url=URL(fileURLWithPath:path)
let writer=try AVAssetWriter(outputURL:url,fileType:.mp4)
let input=AVAssetWriterInput(mediaType:.video,outputSettings:[AVVideoCodecKey:AVVideoCodecType.h264,AVVideoWidthKey:1920,AVVideoHeightKey:1080,AVVideoCompressionPropertiesKey:[AVVideoAverageBitRateKey:700000,AVVideoProfileLevelKey:AVVideoProfileLevelH264MainAutoLevel]])
let adaptor=AVAssetWriterInputPixelBufferAdaptor(assetWriterInput:input,sourcePixelBufferAttributes:[kCVPixelBufferPixelFormatTypeKey as String:kCVPixelFormatType_32ARGB,kCVPixelBufferWidthKey as String:1920,kCVPixelBufferHeightKey as String:1080,kCVPixelBufferIOSurfacePropertiesKey as String:[:]])
writer.add(input);writer.startWriting();writer.startSession(atSourceTime:.zero)
for frame in 0..<150 {
 while !input.isReadyForMoreMediaData {Thread.sleep(forTimeInterval:0.002)}
 var buffer:CVPixelBuffer?;CVPixelBufferPoolCreatePixelBuffer(nil,adaptor.pixelBufferPool!,&buffer)
 let b=buffer!;CVPixelBufferLockBaseAddress(b,[]);let bytes=CVPixelBufferGetBaseAddress(b)!.assumingMemoryBound(to:UInt8.self);let stride=CVPixelBufferGetBytesPerRow(b)
 for y in 0..<1080 {for x in 0..<1920 {let i=y*stride+x*4;bytes[i]=255;bytes[i+1]=UInt8((x+frame*8)%256);bytes[i+2]=UInt8(y%256);bytes[i+3]=UInt8((frame*4)%256)}}
 CVPixelBufferUnlockBaseAddress(b,[]);adaptor.append(b,withPresentationTime:CMTime(value:Int64(frame),timescale:30))
}
input.markAsFinished();let done=DispatchSemaphore(value:0);writer.finishWriting{done.signal()};done.wait();guard writer.status == .completed else {print(writer.error ?? NSError());exit(2)}
print("original H264 Main 1920x1080p30 5 seconds")
