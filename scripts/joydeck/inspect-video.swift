import Foundation
import AVFoundation
import CoreVideo
let asset=AVURLAsset(url:URL(fileURLWithPath:CommandLine.arguments[1]))
let video=asset.tracks(withMediaType:.video),audio=asset.tracks(withMediaType:.audio)
guard let track=video.first else{exit(2)}
let reader=try AVAssetReader(asset:asset)
let output=AVAssetReaderTrackOutput(track:track,outputSettings:[kCVPixelBufferPixelFormatTypeKey as String:kCVPixelFormatType_32BGRA])
reader.add(output);guard reader.startReading()else{exit(3)}
var count=0,first=0.0,last = -1.0,monotonic=true,width=0,height=0
while let sample=output.copyNextSampleBuffer() {
 let pts=CMSampleBufferGetPresentationTimeStamp(sample).seconds
 if count==0 {
  first=pts
  if let image=CMSampleBufferGetImageBuffer(sample) {
   width=CVPixelBufferGetWidth(image);height=CVPixelBufferGetHeight(image)
   CVPixelBufferLockBaseAddress(image,.readOnly)
   let stride=CVPixelBufferGetBytesPerRow(image),base=CVPixelBufferGetBaseAddress(image)!
   var bytes=Data()
   for row in 0..<height {bytes.append(Data(bytes:base.advanced(by:row*stride),count:width*4))}
   try bytes.write(to:URL(fileURLWithPath:CommandLine.arguments[2]))
   CVPixelBufferUnlockBaseAddress(image,.readOnly)
  }
 }
 if pts<=last {monotonic=false};last=pts;count+=1
}
guard reader.status == .completed else{exit(4)}
let report:[String:Any]=["frames":count,"videoTracks":video.count,"audioTracks":audio.count,"firstPts":first,"lastPts":last,"duration":asset.duration.seconds,"width":width,"height":height,"monotonicPts":monotonic]
print(String(data:try JSONSerialization.data(withJSONObject:report,options:[.sortedKeys]),encoding:.utf8)!)
