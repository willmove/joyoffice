import hashlib,json,os,shutil,subprocess,time,zipfile,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/word-native-save-3';OUT.mkdir(exist_ok=False)
SOURCE=OUT/'source.docx';SAVED=OUT/'native 保存.docx';shutil.copyfile(ROOT/'tests/fixtures/docx/DOCX-MIN-001.docx',SOURCE)
env=os.environ.copy();env['JOYWORD_CONFIG_DIR']=str(OUT/'profile')
report={'passed':False,'eventSource':'DDE native file dialog, X11 synthetic keyboard and actual dialog pixels','checks':[]}
log=(OUT/'application.log').open('w');p=subprocess.Popen([str(ROOT/'target/debug/joyword'),str(SOURCE)],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def activate(w):x('windowactivate','--sync',w)
def capture(w,name):subprocess.run(['import','-window',w,str(OUT/name)],check=True,timeout=15)
def text(path):
 with zipfile.ZipFile(path) as z:root=ET.fromstring(z.read('word/document.xml'))
 return ''.join(n.text or '' for n in root.iter('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t'))
try:
 deadline=time.monotonic()+20;app=None
 while time.monotonic()<deadline:
  try:
   ids=x('search','--onlyvisible','--pid',p.pid).splitlines()
   if ids:app=ids[0];break
  except subprocess.CalledProcessError:pass
  time.sleep(.1)
 if not app:raise RuntimeError('no app window')
 activate(app);time.sleep(1);activate(app);x('key','--clearmodifiers','F12')
 deadline=time.monotonic()+15;dialog=None
 while time.monotonic()<deadline:
  active=x('getactivewindow')
  if active!=app:
   try:
    pid=x('getwindowpid',active);comm=Path(f'/proc/{pid}/comm').read_text().strip()
    if comm=='dde-file-dialog':dialog=active;break
   except (OSError,subprocess.CalledProcessError):pass
  time.sleep(.1)
 if not dialog:
  capture(app,'failure-app.png');raise RuntimeError(f'no DDE dialog; active={x("getactivewindow")} app={app}')
 report['dialogWindow']=dialog;report['dialogPid']=x('getwindowpid',dialog);capture(dialog,'01-native-save-dialog.png')
 report['checks'].append('F12 opens a native dde-file-dialog window')
 x('key','--clearmodifiers','ctrl+a');time.sleep(.3);x('type','--clearmodifiers','--delay','12',SAVED.name);capture(dialog,'02-native-path.png');x('mousemove','--window',dialog,1100,640,'click',1)
 deadline=time.monotonic()+60
 while time.monotonic()<deadline and not SAVED.is_file():time.sleep(.1)
 if not SAVED.is_file():
  capture(dialog,'failure-dialog.png')
  processes=subprocess.check_output(['ps','-eo','pid,ppid,stat,wchan:24,comm,args'],text=True).splitlines()
  report['processes']=[line for line in processes if any(name in line for name in ('joyword','dde-file-dialog','xdg-desktop-portal'))]
  raise RuntimeError('native save did not create target after 60 seconds')
 if text(SAVED)!=text(SOURCE):raise AssertionError('saved text differs')
 report['checks'].append('native filename field and Save button write a DOCX with Chinese/space name and independent OOXML readback')
 report['savedSha256']=hashlib.sha256(SAVED.read_bytes()).hexdigest();report['passed']=True
except Exception as e:report['error']=str(e);raise
finally:
 try:
  active=x('getactivewindow')
  if 'dialog' in locals() and dialog and active==dialog:x('key','--clearmodifiers','Escape')
 except subprocess.CalledProcessError:pass
 if p.poll() is None:p.terminate();p.wait(timeout=5)
 log.close();(OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
