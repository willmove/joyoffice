// JoyDeck Windows print adapter. Same owned-helper boundary as JoyOffice's macOS print adapter.
using System;
using System.IO;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Printing;
using System.Web.Script.Serialization;
using System.Windows.Forms;

public static class JoyDeckPrint {
    public class Manifest { public List<string> pages {get;set;} }
    static int current;
    [STAThread]
    public static int Main(string[] args) {
        try {
            Console.OutputEncoding=new System.Text.UTF8Encoding(false);
            if (args.Length!=1) throw new Exception("Expected a local print manifest");
            string manifest=Path.GetFullPath(args[0]);
            if (new FileInfo(manifest).Length>1048576) throw new Exception("Manifest exceeds 1 MiB");
            Manifest m=new JavaScriptSerializer().Deserialize<Manifest>(File.ReadAllText(manifest));
            if (m==null || m.pages==null || m.pages.Count<1 || m.pages.Count>500) throw new Exception("Invalid page count");
            string folder=Path.GetDirectoryName(manifest);
            var files=new List<string>();
            foreach(string name in m.pages) {
                if (name!=Path.GetFileName(name) || !name.EndsWith(".png",StringComparison.OrdinalIgnoreCase)) throw new Exception("Invalid page name");
                string file=Path.GetFullPath(Path.Combine(folder,name));
                if (Path.GetDirectoryName(file)!=folder) throw new Exception("Page outside snapshot");
                using (var input=File.OpenRead(file)) {
                    byte[] header=new byte[24];
                    if (input.Length>67108864 || input.Read(header,0,24)!=24 || header[0]!=137 || header[1]!=80 || header[2]!=78 || header[3]!=71) throw new Exception("Invalid PNG snapshot");
                    uint w=Read32(header,16),h=Read32(header,20);
                    if (w==0 || h==0 || w>8192 || h>8192 || (ulong)w*h>16000000) throw new Exception("Print raster budget exceeded");
                }
                files.Add(file);
            }
            using(var document=new PrintDocument()) using(var dialog=new PrintDialog()) {
                document.DocumentName="JoyDeck presentation";
                using(var first=Image.FromFile(files[0])) document.DefaultPageSettings.Landscape=first.Width>first.Height;
                document.PrintPage+=(sender,e)=> {
                    using(var image=Image.FromFile(files[current])) {
                        Rectangle area=e.MarginBounds;
                        float scale=Math.Min((float)area.Width/image.Width,(float)area.Height/image.Height);
                        float w=image.Width*scale,h=image.Height*scale;
                        e.Graphics.DrawImage(image,area.X+(area.Width-w)/2,area.Y+(area.Height-h)/2,w,h);
                    }
                    current++;e.HasMorePages=current<files.Count;
                };
                dialog.Document=document;dialog.UseEXDialog=true;dialog.AllowPrintToFile=true;
                if(dialog.ShowDialog()!=DialogResult.OK) {Console.WriteLine("cancelled");return 2;}
                document.Print();Console.WriteLine("submitted");return 0;
            }
        } catch(Exception e) {Console.Error.WriteLine(e.Message);return 1;}
    }
    static uint Read32(byte[] b,int i) {return ((uint)b[i]<<24)|((uint)b[i+1]<<16)|((uint)b[i+2]<<8)|b[i+3];}
}
