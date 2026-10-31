"""Capture actual GPUI review/media/output panels; independently inspect generated formats."""
import argparse,hashlib,importlib.util,json,os,subprocess,time,zipfile
from pathlib import Path
from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
spec=importlib.util.spec_from_file_location('bench',SCRIPTS / 'benchmark-m4.py');bench=importlib.util.module_from_spec(spec);spec.loader.exec_module(bench)
def window_list(pid):
 js=f'ObjC.import("CoreGraphics"); JSON.stringify(ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo(1,0))).filter(x=>x.kCGWindowOwnerPID==={pid}));'
 return json.loads(subprocess.check_output(['osascript','-l','JavaScript','-e',js],text=True))
def capture_window(entry,path):
 subprocess.run(['screencapture','-x','-o','-l',str(entry['kCGWindowNumber']),str(path)],check=True)
def print_cancel(pid,out):
 prefix=f'tell application "System Events" to tell (first application process whose unix id is {pid})'
 script=prefix+'''
set frontmost to true
set w to window 1
set g to group 1 of scroll area 2 of splitter group 1 of w
set initialLandscape to value of radio button " Landscape" of g
if initialLandscape is not 1 then error "Landscape default missing"
click radio button "Range from" of g
set value of text field 2 of g to "2"
set value of text field 3 of g to "3"
click radio button " Portrait" of g
if value of radio button " Portrait" of g is not 1 then error "Portrait failed"
click radio button " Landscape" of g
set value of text field 4 of g to "80"
key code 48
set scaleValue to value of text field 4 of g
set firstPage to value of text field 2 of g
set lastPage to value of text field 3 of g
set paperName to value of pop up button 1 of g
return {initialLandscape, firstPage, lastPage, scaleValue, paperName}
end tell'''
 values=subprocess.check_output(['osascript','-e',script],text=True).strip()
 assert values.startswith('1, 2, 3, 80'),values
 capture_window(window_list(pid)[0],out/'print-settings.png')
 tree=subprocess.check_output(['osascript','-e',prefix+' to get entire contents of window 1'],text=True)
 (out/'print-accessibility.txt').write_text(tree)
 subprocess.run(['osascript','-e',prefix+' to click button "Cancel" of splitter group 1 of window 1'],check=True,capture_output=True)
 (out/'print-dialog.json').write_text(json.dumps({'settings':values,'landscapeDefault':True,'orientationToggled':True,'rangeChanged':True,'scalingChanged':True,'cancelClicked':True,'physicalPrint':False},indent=2)+'\n')
 return True
def video_pixels(out):
 from PIL import Image
 import numpy as np
 shot=bench.srgb_image(out/'panel-1.png');frame=Image.open(out/'video-reference.png').convert('RGB')
 # A 165pt contained preview is centered in the panel to the right of the 250pt sidebar.
 window_width=json.loads((out/'panel-1-geometry.json').read_text())['Width']
 dpi=shot.width/window_width
 height=round(165*dpi);width=round(height*frame.width/frame.height)
 reference=np.asarray(frame.resize((width,height),Image.Resampling.BILINEAR),dtype=np.int16)
 x=round((250+window_width)/2*dpi-width/2)
 best=(1e9,None)
 for dx in range(-3,4):
  for y in range(round(220*dpi),round(360*dpi)):
   actual=np.asarray(shot.crop((x+dx,y,x+dx+width,y+height)),dtype=np.int16)
   error=float(np.abs(actual-reference).mean())
   if error<best[0]:best=(error,(x+dx,y,x+dx+width,y+height))
 assert best[0]<8,('Video absent or incorrectly displayed',best)
 shot.crop(best[1]).save(out/'video-preview.png')
 return {'meanRgbError':best[0],'rectangle':best[1],'passed':True}
def main():
 p=argparse.ArgumentParser();p.add_argument('--native-only',action='store_true');p.add_argument('--binary',type=Path,default=binary('joydeck', 'debug'));p.add_argument('--output',type=Path,default=EVIDENCE / 'm12'/f'run-{time.time_ns()}');a=p.parse_args();out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);seen=set();print_done=False;show_done=False
 with (out/'native.log').open('wb') as log:
  proc=subprocess.Popen([str(a.binary.resolve()),'--extended-smoke'],cwd=ROOT,env=dict(os.environ,JOYDECK_M12_OUTPUT=str(out),**({} if a.native_only else {'JOYDECK_M12_GUI':'1'})),stdout=log,stderr=subprocess.STDOUT)
  try:
   deadline=time.monotonic()+140
   while time.monotonic()<deadline:
    text=(out/'native.log').read_text(errors='replace')
    if 'failed=' in text or 'panicked' in text:raise RuntimeError(text[-3000:])
    if not a.native_only and not print_done:
     processes=subprocess.check_output(['ps','-axo','pid=,ppid=,comm='],text=True)
     for row in processes.splitlines():
      parts=row.strip().split(None,2)
      if len(parts)==3 and parts[1]==str(proc.pid) and Path(parts[2]).name=='joydeck-print-macos':
       pid=int(parts[0]);entries=window_list(pid)
       if entries:
        # finishLaunching makes the native print controls available to accessibility.
        count=subprocess.run(['osascript','-e',f'tell application "System Events" to tell (first application process whose unix id is {pid}) to count windows'],capture_output=True,text=True)
        if count.stdout.strip()=='1':print_done=print_cancel(pid,out)
    if 'M12_SHOW_CAPTURE' in text and not show_done:
     entries=[e for e in window_list(proc.pid) if 'Static Slideshow' in e.get('kCGWindowName','')]
     assert len(entries)==1,entries
     capture_window(entries[0],out/'video-show.png');show_done=True;(out/'capture-show.ack').touch()
    for line in text.splitlines():
     if line.startswith('M12_CAPTURE '):
      phase=int(line.split()[1])
      if phase not in seen:
       if not a.native_only:
        windows=bench.windows(proc.pid);assert len(windows)==1,windows
        subprocess.run(['screencapture','-x','-o','-l',str(windows[0]['kCGWindowNumber']),str(out/f'panel-{phase}.png')],check=True)
        (out/f'panel-{phase}-geometry.json').write_text(json.dumps(windows[0]['kCGWindowBounds']))
       seen.add(phase);(out/f'capture-{phase}.ack').touch()
    if proc.poll() is not None:
     assert proc.returncode==0 and 'complete=true' in text,text[-3000:];break
    time.sleep(.1)
   else:raise TimeoutError('GUI validation timeout')
  finally:
   if proc.poll() is None:proc.terminate();proc.wait(timeout=5)
 from pypdf import PdfReader
 pdf=PdfReader(out/'vector.pdf');assert len(pdf.pages)==2
 extracted=''.join(page.extract_text() for page in pdf.pages);assert 'JoyDeck' in extracted or '季度' in extracted,extracted[:500]
 # Standalone chart/shape sample must contain selectable embedded text and path operators.
 fonts=subprocess.check_output(['pdffonts',str(out/'vector.pdf')],text=True);assert 'yes' in fonts,fonts
 subprocess.run(['pdftoppm','-f','1','-singlefile','-scale-to-x','960','-scale-to-y','540','-png',str(out/'vector.pdf'),str(out/'pdf-render')],check=True)
 from PIL import Image
 import numpy as np
 actual=np.asarray(Image.open(out/'pdf-render.png').convert('RGB'),dtype=np.int16);reference=np.asarray(Image.open(out/'pdf-reference.png').convert('RGB'),dtype=np.int16);delta=np.abs(actual-reference);mean=float(delta.mean());assert mean<8,mean
 gif=Image.open(out/'outline.gif');assert gif.n_frames==2 and gif.info['duration']==1000
 for ext,part in [('potx','template'),('ppsx','slideshow')]:
  z=zipfile.ZipFile(out/f'authoring.{ext}');assert f'presentationml.{part}.main+xml'.encode() in z.read('[Content_Types].xml')
 native=zipfile.ZipFile(out/'review-media.joydeck');model=json.loads(native.read('document.json'));assert len(model['review']['threads'])==1;assert any(n.startswith('media/') for n in native.namelist())
 video=None if a.native_only else video_pixels(out)
 if not a.native_only:
  assert print_done and show_done
  assert (out/'print-unchanged.json').exists() and 'cancelled' in (out/'native.log').read_text()
 result={'videoPreview':video,'printCancelGui':print_done,'videoShowCaptured':show_done,'binarySha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'panelsCaptured':0 if a.native_only else len(seen),'nativeOnly':a.native_only,'pdfPages':2,'pdfMeanRgbError':mean,'pdfFonts':fonts,'gifStaticPageFrames':gif.n_frames,'nativeComments':1,'mediaEmbedded':True,'passed':len(seen)==3}
 (out/'verification.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result,ensure_ascii=False,indent=2));print(out)
if __name__=='__main__':main()
