// Owned Windows WPF MediaPlayer adapter. No URL/network input; JSON lines on private stdio.
using System;
using System.IO;
using System.Threading;
using System.Collections.Generic;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;

public static class JoyDeckMedia {
    static readonly JavaScriptSerializer Json=new JavaScriptSerializer {MaxJsonLength=2097152,RecursionLimit=16};
    static MediaPlayer player;
    static bool opened,paused=true;
    static double duration;
    static Dispatcher dispatcher;
    static DispatcherTimer readyTimer;
    static int readyTries;
    static void Send(object value) {Console.WriteLine(Json.Serialize(value));Console.Out.Flush();}
    [STAThread]
    public static int Main(string[] args) {
        try {
            Console.OutputEncoding=new System.Text.UTF8Encoding(false);Console.InputEncoding=new System.Text.UTF8Encoding(false);
            if(args.Length!=1 || !Path.IsPathRooted(args[0]) || args[0].Contains("://")) throw new Exception("Expected an absolute local media file");
            if(new FileInfo(args[0]).Length>67108864) throw new Exception("Media exceeds 64 MiB");
            dispatcher=Dispatcher.CurrentDispatcher;
            player=new MediaPlayer();player.Volume=0;player.ScrubbingEnabled=true;
            player.MediaEnded+=(s,e)=>{paused=true;player.Pause();};
            player.MediaOpened+=(s,e)=> {
                player.Play();
                readyTimer=new DispatcherTimer {Interval=TimeSpan.FromMilliseconds(100)};
                readyTimer.Tick+=(a,b)=> {
                    if(!player.NaturalDuration.HasTimeSpan && ++readyTries<3) return;
                    readyTimer.Stop();
                    if((player.NaturalDuration.HasTimeSpan && player.NaturalDuration.TimeSpan.TotalSeconds>3600) || player.NaturalVideoWidth>8192 || player.NaturalVideoHeight>8192 || (long)player.NaturalVideoWidth*player.NaturalVideoHeight>16000000) {
                        Send(new {error="Unbounded/unsupported media duration or dimensions",naturalDuration=player.NaturalDuration.ToString(),width=player.NaturalVideoWidth,height=player.NaturalVideoHeight});player.Close();dispatcher.BeginInvokeShutdown(DispatcherPriority.Normal);return;
                    }
                    duration=player.NaturalDuration.HasTimeSpan?player.NaturalDuration.TimeSpan.TotalSeconds:0;opened=true;player.Pause();player.Position=TimeSpan.Zero;
                    Send(new {ready=true,width=player.NaturalVideoWidth,height=player.NaturalVideoHeight,duration=duration,durationKnown=player.NaturalDuration.HasTimeSpan,audio=player.HasAudio,video=player.HasVideo,backend="Windows WPF MediaPlayer"});
                };readyTimer.Start();
            };
            player.MediaFailed+=(s,e)=> {Send(new {error=e.ErrorException.Message});player.Close();dispatcher.BeginInvokeShutdown(DispatcherPriority.Normal);};
            player.Open(new Uri(Path.GetFullPath(args[0])));
            var input=new Thread(()=> {
                try {string line;while((line=Console.ReadLine())!=null) {
                    if(line.Length>8192) throw new Exception("Media control line exceeds 8 KiB");
                    var request=Json.Deserialize<Dictionary<string,object>>(line);
                    dispatcher.BeginInvoke(new Action(()=>Handle(request)));
                }}catch(Exception e) {Send(new {error=e.Message});}
                dispatcher.BeginInvoke(new Action(()=> {player.Close();dispatcher.BeginInvokeShutdown(DispatcherPriority.Normal);}));
            });input.IsBackground=true;input.Start();Dispatcher.Run();return 0;
        }catch(Exception e) {Console.Error.WriteLine(e.Message);return 1;}
    }
    static void Handle(Dictionary<string,object> r) {
        try {
            string command=Convert.ToString(r["command"]);
            if(command=="stop") {player.Close();dispatcher.BeginInvokeShutdown(DispatcherPriority.Normal);return;}
            if(!opened) {Send(new {ready=false});return;}
            if(command=="play") {paused=false;player.Play();}
            else if(command=="pause") {paused=true;player.Pause();}
            else if(command=="seek") {
                double at=Convert.ToDouble(r["seconds"]);if(Double.IsNaN(at)||Double.IsInfinity(at)||at<0||at>(duration>0?duration:3600)) throw new Exception("Invalid seek");player.Position=TimeSpan.FromSeconds(at);
                // WPF updates the decoded texture asynchronously, including while paused.
                // Reply on a later dispatcher turn so the next private frame request sees the seek.
                var settled=new DispatcherTimer {Interval=TimeSpan.FromMilliseconds(150)};
                settled.Tick+=(s,e)=>{settled.Stop();Send(new {ready=true,seconds=player.Position.TotalSeconds,paused=paused,duration=duration});};settled.Start();return;
            }
            else if(command=="volume") {double volume=Convert.ToDouble(r["value"]);if(Double.IsNaN(volume)||volume<0||volume>1) throw new Exception("Invalid volume");player.Volume=volume;}
            else if(command!="frame" && command!="status") throw new Exception("Unknown media command");
            if(command=="frame" && player.HasVideo) {
                int w=player.NaturalVideoWidth,h=player.NaturalVideoHeight;
                double k=Math.Min(1,Math.Min(640.0/Math.Max(1,w),360.0/Math.Max(1,h)));w=Math.Max(1,(int)(w*k));h=Math.Max(1,(int)(h*k));
                var drawing=new DrawingVisual();using(var dc=drawing.RenderOpen()) {dc.DrawVideo(player,new Rect(0,0,w,h));}
                var target=new RenderTargetBitmap(w,h,96,96,PixelFormats.Pbgra32);target.Render(drawing);
                byte[] pixels=new byte[w*h*4];target.CopyPixels(pixels,w*4,0);
                Send(new {ready=true,seconds=player.Position.TotalSeconds,paused=paused,width=w,height=h,bgra=Convert.ToBase64String(pixels)});
            }else Send(new {ready=true,seconds=player.Position.TotalSeconds,paused=paused,duration=duration});
        }catch(Exception e) {Send(new {error=e.Message});}
    }
}
