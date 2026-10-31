// JoyDeck owned-process AVFoundation adapter. No microphone/camera and no document access.
import Foundation
import AVFoundation
import CoreImage
import AppKit
func send(_ value: [String:Any]) { if let data = try? JSONSerialization.data(withJSONObject:value, options:[.sortedKeys]) { FileHandle.standardOutput.write(data); FileHandle.standardOutput.write(Data([10])) } }
let args = CommandLine.arguments
if args.count < 3 { exit(2) }
let asset = AVURLAsset(url: URL(fileURLWithPath:args[2]))
func probe() async throws -> [String:Any] {
 let duration = try await asset.load(.duration).seconds
 let tracks = try await asset.load(.tracks)
 var codecs:[String] = []; var width=0; var height=0;var profiles:[String]=[];var fps:Float=0
 for t in tracks {
  for f in try await t.load(.formatDescriptions) {
   if CMFormatDescriptionGetMediaSubType(f)==kCMVideoCodecType_H264 {
    let extensions=(CMFormatDescriptionGetExtensions(f) as NSDictionary?) ?? NSDictionary()
    guard let atoms=extensions[kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms] as? NSDictionary,let data=atoms["avcC"] as? Data,data.count>3,[66,77].contains(Int(data[1])) else {throw NSError(domain:"unsupported-profile",code:2,userInfo:[NSLocalizedDescriptionKey:"Only 8-bit H264 Baseline/Main profiles are enabled"])}
    profiles.append(data[1]==66 ? "H264-Baseline-8bit" : "H264-Main-8bit")
   }
   let c=CMFormatDescriptionGetMediaSubType(f);codecs.append(String(bytes:[UInt8((c>>24)&255),UInt8((c>>16)&255),UInt8((c>>8)&255),UInt8(c&255)],encoding:.ascii) ?? "unknown")}
  if t.mediaType == .video {let size=try await t.load(.naturalSize);width=Int(size.width);height=Int(size.height);fps=try await t.load(.nominalFrameRate)}
 }
 let ext=URL(fileURLWithPath:args[2]).pathExtension.lowercased()
 let allowed = (ext=="wav" && codecs.allSatisfy{$0=="lpcm"}) || (ext=="mp3" && codecs.allSatisfy{$0==".mp3"}) || (ext=="mp4" && codecs.contains("avc1") && codecs.allSatisfy{$0=="avc1" || $0=="aac "})
 guard duration.isFinite && duration>0 && duration<=3600 && width<=1920 && height<=1080 && fps<=30.01 && !codecs.isEmpty && allowed else {throw NSError(domain:"unsupported",code:1,userInfo:[NSLocalizedDescriptionKey:"Only PCM WAV, MP3, H.264/AAC MP4 <=1080p <=1h supported"])}
 return ["duration":duration,"codecs":codecs,"width":width,"height":height,"adapter":"AVFoundation-macOS","profiles":profiles,"fps":fps]
}
if args[1]=="probe" {
 Task {do{send(try await probe());exit(0)}catch{send(["error":error.localizedDescription]);exit(3)}}
 RunLoop.main.run()
} else if args[1]=="play" && args.count>=9 {
 let directory=URL(fileURLWithPath:args[3],isDirectory:true)
 let start=Double(args[4]) ?? 0;let end=Double(args[5]) ?? 0;let volume=Float(args[6]) ?? 1;let looping=args[7]=="true";let autoplay=args[8]=="true"
 var permitted=false
 Task {do{_=try await probe();permitted=true}catch{send(["error":error.localizedDescription]);exit(3)}}
 let item=AVPlayerItem(asset:asset);item.preferredForwardBufferDuration=0.5
 let output=AVPlayerItemVideoOutput(pixelBufferAttributes:[kCVPixelBufferPixelFormatTypeKey as String:kCVPixelFormatType_32BGRA]);item.add(output)
 let player=AVPlayer(playerItem:item);player.volume=volume;player.actionAtItemEnd = .pause
 let context=CIContext(options:[.cacheIntermediates:false]);var generation=0;var sequence=0;var seeking=false;var started=false;var ended=false
 func seek(_ seconds:Double,_ resume:Bool){generation+=1;let g=generation;seeking=true;player.pause();player.seek(to:CMTime(seconds:max(start,min(end,seconds)),preferredTimescale:600),toleranceBefore:.zero,toleranceAfter:.zero){done in DispatchQueue.main.async {if generation==g {seeking=false;ended=false;if done && resume {player.play()}}}}}
 DispatchQueue.global(qos:.userInitiated).async {
  while let line=readLine(){guard let data=line.data(using:.utf8),let v=(try? JSONSerialization.jsonObject(with:data)) as? [String:Any],let command=v["command"] as? String else {continue}
   DispatchQueue.main.async {switch command {
    case "play":if ended {seek(start,true)}else{player.play()}
    case "pause":player.pause()
    case "seek":seek(v["seconds"] as? Double ?? start,player.rate>0)
    case "volume":player.volume=max(0,min(1,v["value"] as? Float ?? 1))
    case "stop":player.pause();exit(0)
    default:break
   }}
  };DispatchQueue.main.async {player.pause();exit(0)}
 }
 let timer=Timer.scheduledTimer(withTimeInterval:1.0/30.0,repeats:true){_ in autoreleasepool {
  if !permitted{return}
  if item.status == .failed {send(["error":item.error?.localizedDescription ?? "decode-failed"]);exit(4)}
  if !started && item.status == .readyToPlay {started=true;seek(start,autoplay)}
  let time=player.currentTime().seconds
  if started && !seeking && time>=end-0.015 && !ended {if looping {seek(start,true)}else{ended=true;player.pause()}}
  var frame=false
  if !seeking && output.hasNewPixelBuffer(forItemTime:player.currentTime()),let buffer=output.copyPixelBuffer(forItemTime:player.currentTime(),itemTimeForDisplay:nil) {
   let image=CIImage(cvPixelBuffer:buffer);let scale=min(1,960/image.extent.width);let small=image.transformed(by:CGAffineTransform(scaleX:scale,y:scale));let width=Int(small.extent.width);let height=Int(small.extent.height)
   if width>0 && height>0 && width*height<=960*960 {
    var bytes=[UInt8](repeating:0,count:width*height*4)
    context.render(small,toBitmap:&bytes,rowBytes:width*4,bounds:CGRect(x:0,y:0,width:width,height:height),format:.BGRA8,colorSpace:CGColorSpaceCreateDeviceRGB())
    var w=UInt32(width).littleEndian;var h=UInt32(height).littleEndian;var g=UInt32(generation).littleEndian
    var data=Data(bytes:&w,count:4);data.append(Data(bytes:&h,count:4));data.append(Data(bytes:&g,count:4));data.append(contentsOf:bytes)
    do{try data.write(to:directory.appendingPathComponent("frame.bgra"),options:.atomic);sequence+=1;frame=true}catch{send(["error":"frame-write-failed"]);exit(5)}
   }
  }
  send(["seconds":time.isFinite ? time : start,"playing":player.rate>0,"ready":started,"ended":ended,"generation":generation,"sequence":sequence,"frame":frame])
 }}
 RunLoop.main.add(timer,forMode:.common);RunLoop.main.run()
} else {exit(2)}
