import Foundation
import AVFoundation
let root=CommandLine.arguments[1]
Task {
 do {
  let video=AVURLAsset(url:URL(fileURLWithPath:root+"/original-h264.mp4"));let audio=AVURLAsset(url:URL(fileURLWithPath:root+"/original-tone.m4a"));let composition=AVMutableComposition()
  let v=try await video.loadTracks(withMediaType:.video)[0];let a=try await audio.loadTracks(withMediaType:.audio)[0];let range=CMTimeRange(start:.zero,duration:CMTime(seconds:5,preferredTimescale:600))
  try composition.addMutableTrack(withMediaType:.video,preferredTrackID:kCMPersistentTrackID_Invalid)!.insertTimeRange(range,of:v,at:.zero)
  try composition.addMutableTrack(withMediaType:.audio,preferredTrackID:kCMPersistentTrackID_Invalid)!.insertTimeRange(range,of:a,at:.zero)
  let exporter=AVAssetExportSession(asset:composition,presetName:AVAssetExportPresetPassthrough)!
  try await exporter.export(to:URL(fileURLWithPath:root+"/original-h264-aac.mp4"),as:.mp4);exit(0)
 }catch{print(error);exit(2)}
}
RunLoop.main.run()
