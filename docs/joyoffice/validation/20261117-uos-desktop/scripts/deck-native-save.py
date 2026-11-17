import hashlib,json,os,shutil,subprocess,time,zipfile,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/deck-native-save-3';OUT.mkdir(exist_ok=False)
SOURCE=OUT/'source.pptx';SAVED=OUT/'native 保存.pptx';shutil.copyfile(ROOT/'tests/fixtures/joydeck/r1-preview.pptx',SOURCE)
env=os.environ.copy();env['XDG_STATE_HOME']=str(OUT/'state');report={'passed':False,'eventSource':'DDE native file dialog and X11 synthetic keyboard/pointer','checks':[]}
log=(OUT/'application.log').open('w');p=subprocess.Popen([str(ROOT/'target/debug/joydeck'),str(SOURCE)],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def capture(w,name):subprocess.run(['import','-window',w,str(OUT/name)],check=True,timeout=15)
try:
 deadline=time.monotonic()+20;app=None
 while time.monotonic()<deadline:
  try:
   ids=x('search','--onlyvisible','--pid',p.pid).splitlines()
   if ids:app=ids[0];break
  except subprocess.CalledProcessError:pass
  time.sleep(.1)
 if not app:raise RuntimeError('no app window')
 x('windowactivate','--sync',app);time.sleep(1);x('mousemove','--window',app,30,55,'click',1);time.sleep(.7);capture(app,'01-file-page.png');x('mousemove','--window',app,85,653,'click',1)
 deadline=time.monotonic()+15;dialog=None
 while time.monotonic()<deadline:
  active=x('getactivewindow')
  if active!=app:
   try:
    pid=x('getwindowpid',active);comm=Path(f'/proc/{pid}/comm').read_text().strip()
    if comm=='dde-file-dialog':dialog=active;break
   except (OSError,subprocess.CalledProcessError):pass
  time.sleep(.1)
 if not dialog:capture(app,'failure-app.png');raise RuntimeError('no DDE dialog')
 report['dialogWindow']=dialog;report['dialogPid']=x('getwindowpid',dialog);capture(dialog,'01-native-save-dialog.png');report['checks'].append('File > Save As opens a native dde-file-dialog window')
 x('key','--clearmodifiers','ctrl+a');x('type','--clearmodifiers','--delay','12',SAVED.name);capture(dialog,'02-native-path.png');x('mousemove','--window',dialog,1100,640,'click',1)
 deadline=time.monotonic()+30
 while time.monotonic()<deadline and not SAVED.is_file():time.sleep(.1)
 if not SAVED.is_file():capture(dialog,'failure-dialog.png');raise RuntimeError('native save did not create target after 30 seconds')
 with zipfile.ZipFile(SAVED) as z:
  root=ET.fromstring(z.read('ppt/presentation.xml'));assert len(root.findall('.//{http://schemas.openxmlformats.org/presentationml/2006/main}sldId'))==3
 report['checks'].append('native filename and Save button create a valid three-page PPTX');report['savedSha256']=hashlib.sha256(SAVED.read_bytes()).hexdigest();report['passed']=True
except Exception as e:report['error']=str(e);raise
finally:
 try:
  if 'dialog' in locals() and dialog and x('getactivewindow')==dialog:x('key','--clearmodifiers','Escape')
 except subprocess.CalledProcessError:pass
 if p.poll() is None:p.terminate();p.wait(timeout=5)
 log.close();(OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
