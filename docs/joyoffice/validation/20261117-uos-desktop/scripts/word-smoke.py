"""JoyWord UOS desktop smoke with isolated files, live control and X11 input."""
import hashlib,json,os,secrets,shutil,socket,subprocess,time,zipfile,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/word-native-2';OUT.mkdir(exist_ok=False)
BINARY=ROOT/'target/debug/joyword';FIXTURE=ROOT/'tests/fixtures/docx/DOCX-MIN-001.docx'
SOURCE=OUT/'中文 空格 测试.docx';shutil.copyfile(FIXTURE,SOURCE)
original=hashlib.sha256(FIXTURE.read_bytes()).hexdigest();key=secrets.token_hex(32)
env=os.environ.copy();env.update(JOYWORD_CONFIG_DIR=str(OUT/'profile'),JOYWORD_CONTROL_KEY=key)
report={'passed':False,'binarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'checks':[],
        'eventSource':'X11 OS synthetic keys, actual window pixels and authenticated live document control',
        'nativeImeVerified':False};process=None;port=None;window=None

def wait(fn,label):
 deadline=time.monotonic()+35;last=None
 while time.monotonic()<deadline:
  if process.poll() is not None:raise RuntimeError(f'Application exited {process.returncode}: {label}')
  try:
   result=fn()
   if result:return result
  except (OSError,ValueError,subprocess.SubprocessError) as e:last=str(e)
  time.sleep(.1)
 raise TimeoutError(f'{label}: {last}')

def call(method,**params):
 with socket.create_connection(('127.0.0.1',port),timeout=3) as client:
  client.settimeout(5);client.sendall((json.dumps({'id':1,'method':method,'params':params,'key':key})+'\n').encode())
  result=json.loads(client.makefile('rb').readline())
 if not result.get('ok'):raise ValueError(str(result))
 return result['result']

def command(name,**params):
 state=call('document.inspect')
 return call('engine.execute',command=name,params=params,doc=state['doc'],version=state['version'])

def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def keys(*args):x('windowactivate','--sync',window);x('key','--clearmodifiers',*args);time.sleep(.2)
def capture(name):
 x('windowactivate','--sync',window);time.sleep(.3)
 subprocess.run(['import','-window',window,str(OUT/name)],check=True,timeout=15)
def saved_text():
 with zipfile.ZipFile(SOURCE) as z:root=ET.fromstring(z.read('word/document.xml'))
 return ''.join(n.text or '' for n in root.iter('{http://schemas.openxmlformats.org/wordprocessingml/2006/main}t'))
def launch(index):
 global port,process,window
 with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
 log=(OUT/f'application-{index}.log').open('w')
 process=subprocess.Popen([str(BINARY),'--control',str(port),'--',str(SOURCE)],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT);log.close()
 wait(lambda:call('document.inspect'),'document load')
 window=wait(lambda:x('search','--onlyvisible','--pid',process.pid).splitlines()[0],'visible window')
 x('windowactivate','--sync',window);time.sleep(.5)
def close():
 global process
 if process and process.poll() is None:
  keys('alt+F4')
  try:process.wait(timeout=8)
  except subprocess.TimeoutExpired:process.terminate();process.wait(timeout=5)
 process=None
try:
 launch(1);before=call('document.inspect')['text'];capture('01-open.png')
 report['checks'].append('actual X11 window opens the isolated Chinese/space DOCX path')
 command('caret.docEnd');x('type','--clearmodifiers','--delay','15',' UOS20261117-X11')
 wait(lambda:'UOS20261117-X11' in call('document.inspect')['text'],'OS text input');edited=call('document.inspect')
 keys('ctrl+z');wait(lambda:call('document.inspect')['text']!=edited['text'],'OS undo')
 keys('ctrl+shift+z');wait(lambda:call('document.inspect')['text']==edited['text'],'OS redo')
 report['checks'].append('OS keyboard typing, undo and redo reach the live editing session')
 keys('ctrl+f');x('type','--clearmodifiers','--delay','20','UOS20261117');capture('02-search-input.png');keys('Escape');capture('03-after-escape.png')
 assert call('document.inspect')['text']==edited['text']
 report['checks'].append('Kit search input accepts text without changing document content')
 keys('ctrl+s');capture('04-metadata-warning.png');x('mousemove','--window',window,184,266,'click',1);wait(lambda:'UOS20261117-X11' in saved_text(),'save and independent OOXML readback');capture('05-saved.png');close()
 report['checks'].append('OS Save with explicit metadata replacement confirmation writes only the disposable DOCX')
 launch(2);assert 'UOS20261117-X11' in call('document.inspect')['text'];capture('06-reopened.png');close()
 assert hashlib.sha256(FIXTURE.read_bytes()).hexdigest()==original
 report['checks'].append('fresh desktop process reopens saved text; repository fixture hash is unchanged')
 report['passed']=True
except Exception as error:
 report['error']=str(error);raise
finally:
 if process and process.poll() is None:process.terminate();process.wait(timeout=5)
 (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
