import hashlib,json,os,secrets,shutil,socket,subprocess,time,zipfile,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/word-save-dialog-native';OUT.mkdir(exist_ok=False)
BINARY=ROOT/'target/debug/joyword';SOURCE=OUT/'dialog-test.docx';shutil.copyfile(ROOT/'assets/samples/letter.docx',SOURCE)
key=secrets.token_hex(32);env=os.environ.copy();env.update(JOYWORD_CONFIG_DIR=str(OUT/'profile'),JOYWORD_CONTROL_KEY=key)
report={'passed':False,'binarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'eventSource':'X11 OS synthetic keyboard/pointer, actual window pixels and independent OOXML readback','checks':[]}
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
log=(OUT/'application.log').open('w');p=subprocess.Popen([str(BINARY),'--control',str(port),'--',str(SOURCE)],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT);log.close()
def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def call(method,**params):
 with socket.create_connection(('127.0.0.1',port),timeout=3) as client:
  client.settimeout(5);client.sendall((json.dumps({'id':1,'method':method,'params':params,'key':key})+'\n').encode());reply=json.loads(client.makefile('rb').readline())
 if not reply.get('ok'):raise ValueError(str(reply))
 return reply['result']
def command(name,**params):
 state=call('document.inspect');return call('engine.execute',command=name,params=params,doc=state['doc'],version=state['version'])
def wait(fn,label):
 deadline=time.monotonic()+30;last=None
 while time.monotonic()<deadline:
  if p.poll() is not None:raise RuntimeError(f'JoyWord exited {p.returncode}: {label}')
  try:
   value=fn()
   if value:return value
  except (OSError,ValueError,subprocess.CalledProcessError) as e:last=str(e)
  time.sleep(.1)
 raise TimeoutError(f'{label}: {last}')
def keys(*args):x('windowactivate','--sync',window);x('key','--clearmodifiers',*args);time.sleep(.3)
def capture(name):x('windowactivate','--sync',window);time.sleep(.3);subprocess.run(['import','-window',window,str(OUT/name)],check=True,timeout=15)
def saved_text():
 with zipfile.ZipFile(SOURCE) as z:root=ET.fromstring(z.read('word/document.xml'))
 return ''.join(n.text or '' for n in root.iter('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t'))
try:
 wait(lambda:call('document.inspect'),'source load');window=wait(lambda:x('search','--onlyvisible','--pid',p.pid).splitlines()[0],'visible window')
 x('windowactivate','--sync',window);time.sleep(1);command('caret.docEnd');x('type','--clearmodifiers','--delay','12',' UOS save dialog 20261117')
 wait(lambda:'UOS save dialog 20261117' in call('document.inspect')['text'],'dirty edit')
 keys('ctrl+n');capture('01-save-changes.png');keys('Escape');assert 'UOS save dialog 20261117' in call('document.inspect')['text']
 report['checks'].append('centered unsaved-New dialog cancels with Escape and retains the edit')
 keys('ctrl+n');keys('Return');capture('02-replace-original.png');keys('Return')
 wait(lambda:'UOS save dialog 20261117' in saved_text(),'save and replacement confirmation')
 wait(lambda:'UOS save dialog 20261117' not in call('document.inspect')['text'],'new document after save')
 capture('03-new-after-save.png');assert 'Discard validation' not in saved_text()
 report['checks'].append('Enter saves and confirms original replacement inside centered dialogs; pending New continues')
 command('caret.docEnd');x('type','--clearmodifiers','--delay','12','Discard validation')
 wait(lambda:'Discard validation' in call('document.inspect')['text'],'second dirty edit');keys('ctrl+n');capture('04-discard-dialog.png')
 x('mousemove','--window',window,1040,620,'click',1);wait(lambda:'Discard validation' not in call('document.inspect')['text'],'discard and new')
 assert 'Discard validation' not in saved_text();capture('05-new-after-discard.png')
 report['checks'].append('Discard starts another blank document without writing the discarded text')
 report['savedSha256']=hashlib.sha256(SOURCE.read_bytes()).hexdigest();report['passed']=True
except Exception as e:report['error']=str(e);raise
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=5)
 (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
