// Static slideshow MP4 via Windows MediaComposition. Uses local snapshot PNGs, no audio/notes.
using System;
using System.IO;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using Windows.Storage;
using Windows.Media.Editing;
using Windows.Media.MediaProperties;
using Windows.Media.Transcoding;
public static class JoyDeckVideo {
    public class Manifest {public List<string> pages{get;set;}public double seconds{get;set;}public uint fps{get;set;}}
    [MTAThread]
    public static int Main(string[] args) {
        Console.OutputEncoding=new System.Text.UTF8Encoding(false);
        try {
            if(args.Length!=1)throw new Exception("Expected local video manifest");
            string path=Path.GetFullPath(args[0]),folder=Path.GetDirectoryName(path);
            if(new FileInfo(path).Length>1048576)throw new Exception("Manifest exceeds budget");
            var manifest=new JavaScriptSerializer().Deserialize<Manifest>(File.ReadAllText(path));
            if(manifest.pages==null||manifest.pages.Count==0||manifest.pages.Count>120||Double.IsNaN(manifest.seconds)||manifest.seconds<.1||manifest.seconds>30||manifest.seconds*manifest.pages.Count>60)throw new Exception("MP4 duration/pages exceed budget");
            var composition=new MediaComposition();uint width=0,height=0;
            foreach(string name in manifest.pages) {
                if(name!=Path.GetFileName(name)||!name.EndsWith(".png",StringComparison.OrdinalIgnoreCase))throw new Exception("Invalid local snapshot");
                string png=Path.GetFullPath(Path.Combine(folder,name));if(Path.GetDirectoryName(png)!=folder)throw new Exception("Page outside snapshot");
                if(new FileInfo(png).Length>67108864)throw new Exception("PNG exceeds budget");
                using(var input=File.OpenRead(png)) {byte[] header=new byte[24];if(input.Read(header,0,24)!=24||header[0]!=137||header[1]!=80||header[2]!=78||header[3]!=71)throw new Exception("Invalid PNG");
                    uint w=Read32(header,16),h=Read32(header,20);if(w==0||h==0||w>8192||h>8192||(ulong)w*h>16000000)throw new Exception("Raster exceeds budget");if(width==0){width=w;height=h;}}
                var file=StorageFile.GetFileFromPathAsync(png).AsTask().GetAwaiter().GetResult();
                var clip=MediaClip.CreateFromImageFileAsync(file,TimeSpan.FromSeconds(manifest.seconds)).AsTask().GetAwaiter().GetResult();composition.Clips.Add(clip);
            }
            var output=StorageFolder.GetFolderFromPathAsync(folder).AsTask().GetAwaiter().GetResult().CreateFileAsync("video.mp4",CreationCollisionOption.FailIfExists).AsTask().GetAwaiter().GetResult();
            var profile=MediaEncodingProfile.CreateMp4(VideoEncodingQuality.HD720p);profile.Audio=null;
            double scale=Math.Min(1,Math.Min(1280.0/width,720.0/height));profile.Video.Width=Math.Max(2,((uint)(width*scale)/2)*2);profile.Video.Height=Math.Max(2,((uint)(height*scale)/2)*2);profile.Video.Bitrate=4000000;
            if(manifest.fps<1||manifest.fps>30)throw new Exception("FPS exceeds budget");profile.Video.FrameRate.Numerator=manifest.fps;profile.Video.FrameRate.Denominator=1;
            var operation=composition.RenderToFileAsync(output,MediaTrimmingPreference.Precise,profile);
            operation.Progress=(sender,progress)=>{Console.WriteLine("{\"progress\":"+(progress/100.0).ToString(System.Globalization.CultureInfo.InvariantCulture)+"}");Console.Out.Flush();};
            var result=operation.AsTask().GetAwaiter().GetResult();if(result!=TranscodeFailureReason.None)throw new Exception("Windows encoder: "+result);
            Console.WriteLine("{\"completed\":true,\"audio\":false,\"mode\":\"static-slides\"}");return 0;
        }catch(Exception e){Console.Error.WriteLine(e.Message);return 1;}
    }
    static uint Read32(byte[] b,int i){return ((uint)b[i]<<24)|((uint)b[i+1]<<16)|((uint)b[i+2]<<8)|b[i+3];}
}
