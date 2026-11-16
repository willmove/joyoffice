"""Current R9 desktop stdio and X11 regression; uses only disposable files."""
import hashlib,json,os,re,select,shutil,subprocess,time,zipfile,xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/gpui-kit-091/deck-native';OUT.mkdir(exist_ok=False)
BINARY=ROOT/'target/debug/joydeck';FIXTURE=ROOT/'tests/fixtures/joydeck/r1-preview.pptx'
SOURCE=OUT/'中文 空格 演示.pptx';SAVED=OUT/'保存 重开.pptx';shutil.copyfile(FIXTURE,SOURCE)
original=hashlib.sha256(FIXTURE.read_bytes()).hexdigest();env=os.environ.copy();env['XDG_STATE_HOME']=str(OUT/'state');env.pop('LOCALAPPDATA',None)
report={'passed':False,'binarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'checks':[],
        'eventSource':'root-scoped desktop stdio, X11 OS synthetic keys and actual window pixels',
        'nativeImeVerified':False,'windowsUiaVerified':False};process=None;window=None;request_id=0

def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def wait(fn,label):
 deadline=time.monotonic()+35;last=None
 while time.monotonic()<deadline:
  if process.poll() is not None:raise RuntimeError(f'Deck exited {process.returncode}: {label}')
  try:
   value=fn()
   if value:return value
  except (OSError,ValueError,subprocess.SubprocessError) as e:last=str(e)
  time.sleep(.1)
 raise TimeoutError(f'{label}: {last}')
def call(method,params=None,mutating=False):
 global request_id
 status=call('document.status') if mutating or method=='document.inspect' else None
 request_id+=1;r={'protocolVersion':'joydeck-control-1','requestId':f'gpui091-{request_id}','method':method,'params':params or {}}
 if status:r.update(target=status['documentId'],expectedRevision=status.get('revision'))
 process.stdin.write(json.dumps(r,ensure_ascii=False)+'\n');process.stdin.flush()
 if not select.select([process.stdout],[],[],10)[0]:raise TimeoutError('desktop stdio response')
 response=json.loads(process.stdout.readline())
 if 'error' in response:raise ValueError(str(response['error']))
 return response['result']
def keys(*args):x('windowactivate','--sync',window);x('key','--clearmodifiers',*args);time.sleep(.2)
def capture(name):
 x('windowactivate','--sync',window);time.sleep(.4)
 subprocess.run(['import','-window',window,str(OUT/name)],check=True,timeout=15)
def launch(source,index,dark=False):
 global process,window
 log=(OUT/f'application-{index}.log').open('w')
 process=subprocess.Popen([str(BINARY),str(source),f'--control-stdio={OUT}',*(['--dark'] if dark else [])],cwd=ROOT,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True,encoding='utf-8',bufsize=1);log.close()
 wait(lambda:call('document.status').get('pages'),'document load')
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
 launch(SOURCE,1);caps=call('engine.commands');assert caps['attachedDesktop'] and caps['commandSchemaVersion']==2
 n=call('document.status')['pages'];capture('01-open.png');report['checks'].append('current R9 desktop window opens a Chinese/space PPTX path; live schema 2 attached editor')
 valid=call('command.validate',{'command':{'command':'duplicate-slide'}},True);assert valid['valid'] and valid['wouldChange'];assert call('document.status')['pages']==n
 call('command.execute',{'command':{'command':'duplicate-slide'}},True);assert call('document.status')['pages']==n+1
 report['checks'].append('validation preserves live state; duplicate-slide updates the actual editor')
 keys('Escape','ctrl+z');wait(lambda:call('document.status')['pages']==n,'OS undo')
 keys('ctrl+y');wait(lambda:call('document.status')['pages']==n+1,'OS redo')
 report['checks'].append('X11 OS keyboard undo/redo follows desktop changes')
 call('document.save',{'path':str(SAVED)},True)
 with zipfile.ZipFile(SAVED) as z:
  presentation=ET.fromstring(z.read('ppt/presentation.xml'));assert len(presentation.findall('.//{http://schemas.openxmlformats.org/presentationml/2006/main}sldId'))==n+1
 capture('02-saved.png');close();report['checks'].append('PPTX save independently contains the added slide')
 launch(SAVED,2,True);assert call('document.status')['pages']==n+1;capture('03-dark-reopened.png');close()
 assert hashlib.sha256(FIXTURE.read_bytes()).hexdigest()==original
 report['checks'].append('fresh dark-theme desktop reopens saved slides; repository fixture hash is unchanged')
 report['passed']=True
except Exception as error:report['error']=str(error);raise
finally:
 if process and process.poll() is None:process.terminate();process.wait(timeout=5)
 (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
