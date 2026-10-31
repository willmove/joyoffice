import Foundation
import AVFoundation
import CoreVideo

func fail(_ message:String)->Never {FileHandle.standardError.write(Data((message+"\n").utf8));exit(2)}
func exact(_ count:Int)->Data? {
 var data=Data()
 while data.count<count {
  guard let next=try? FileHandle.standardInput.read(upToCount:count-data.count),!next.isEmpty else{return nil}
  data.append(next)
 }
 return data
}
guard CommandLine.arguments.count==2,let header=exact(16)else{fail("invalid header")}
func number(_ offset:Int)->Int {Int(header[offset]) | Int(header[offset+1])<<8 | Int(header[offset+2])<<16 | Int(header[offset+3])<<24}
let width=number(0),height=number(4),fps=number(8),frames=number(12)
guard (320...1920).contains(width),(2...1080).contains(height),width%2==0,height%2==0,(1...30).contains(fps),(1...1800).contains(frames)else{fail("budget exceeded")}
let url=URL(fileURLWithPath:CommandLine.arguments[1])
guard !FileManager.default.fileExists(atPath:url.path)else{fail("output exists")}
let writer:AVAssetWriter
do{writer=try AVAssetWriter(outputURL:url,fileType:.mp4)}catch{fail("cannot create encoder")}
let input=AVAssetWriterInput(mediaType:.video,outputSettings:[AVVideoCodecKey:AVVideoCodecType.h264,AVVideoWidthKey:width,AVVideoHeightKey:height,AVVideoCompressionPropertiesKey:[AVVideoAverageBitRateKey:2000000,AVVideoProfileLevelKey:AVVideoProfileLevelH264MainAutoLevel],AVVideoColorPropertiesKey:[AVVideoColorPrimariesKey:AVVideoColorPrimaries_ITU_R_709_2,AVVideoTransferFunctionKey:AVVideoTransferFunction_ITU_R_709_2,AVVideoYCbCrMatrixKey:AVVideoYCbCrMatrix_ITU_R_709_2]])
input.expectsMediaDataInRealTime=false
let adaptor=AVAssetWriterInputPixelBufferAdaptor(assetWriterInput:input,sourcePixelBufferAttributes:[kCVPixelBufferPixelFormatTypeKey as String:kCVPixelFormatType_32BGRA,kCVPixelBufferWidthKey as String:width,kCVPixelBufferHeightKey as String:height,kCVPixelBufferIOSurfacePropertiesKey as String:[:]])
guard writer.canAdd(input)else{fail("unsupported settings")};writer.add(input)
guard writer.startWriting()else{fail("cannot start")};writer.startSession(atSourceTime:.zero)
for index in 0..<frames {
 guard let bytes=exact(width*height*4)else{writer.cancelWriting();fail("truncated frame")}
 while !input.isReadyForMoreMediaData {if writer.status != .writing{fail("encoder failed")};Thread.sleep(forTimeInterval:0.002)}
 var buffer:CVPixelBuffer?
 guard let pool=adaptor.pixelBufferPool,CVPixelBufferPoolCreatePixelBuffer(nil,pool,&buffer)==kCVReturnSuccess,let pixel=buffer else{writer.cancelWriting();fail("pixel budget")}
 CVPixelBufferLockBaseAddress(pixel,[])
 let stride=CVPixelBufferGetBytesPerRow(pixel),base=CVPixelBufferGetBaseAddress(pixel)!
 bytes.withUnsafeBytes {source in for row in 0..<height {base.advanced(by:row*stride).copyMemory(from:source.baseAddress!.advanced(by:row*width*4),byteCount:width*4)}}
 CVPixelBufferUnlockBaseAddress(pixel,[])
 guard adaptor.append(pixel,withPresentationTime:CMTime(value:Int64(index),timescale:Int32(fps)))else{writer.cancelWriting();fail("append failed")}
}
writer.endSession(atSourceTime:CMTime(value:Int64(frames),timescale:Int32(fps)))
input.markAsFinished();let done=DispatchSemaphore(value:0);writer.finishWriting{done.signal()};done.wait()
guard writer.status == .completed else{fail("finish failed")}
