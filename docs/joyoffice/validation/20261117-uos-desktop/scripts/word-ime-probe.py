import json,os,secrets,shutil,socket,subprocess,time
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/word-ime-native';OUT.mkdir(exist_ok=False)
SOURCE=OUT/'ime.docx';shutil.copyfile(ROOT/'tests/fixtures/docx/DOCX-MIN-001.docx',SOURCE)
key=secrets.token_hex(32);env=os.environ.copy();env.update(JOYWORD_CONFIG_DIR=str(OUT/'profile'),JOYWORD_CONTROL_KEY=key)
report={'passed':False,'probeCompleted':False,'nativeImeVerified':False,'inputMethod':'com.sogou.ime.ng.fcitx5.uos','eventSource':'X11 OS synthetic ASCII keys through the active system input method'}
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
log=(OUT/'application.log').open('w');p=subprocess.Popen([str(ROOT/'target/debug/joyword'),'--control',str(port),'--',str(SOURCE)],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT);log.close()
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
def capture(name,root_crop=False):
 if root_crop:
  values={}
  for line in x('getwindowgeometry','--shell',window).splitlines():
   k,v=line.split('=',1);values[k]=v
  subprocess.run(['import','-window','root','-crop',f'{values["WIDTH"]}x{values["HEIGHT"]}+{values["X"]}+{values["Y"]}',str(OUT/name)],check=True,timeout=15)
 else:subprocess.run(['import','-window',window,str(OUT/name)],check=True,timeout=15)
try:
 subprocess.run(['fcitx5-remote','-o'],check=True);wait(lambda:call('document.inspect'),'document load')
 window=wait(lambda:x('search','--onlyvisible','--pid',p.pid).splitlines()[0],'visible window');x('windowactivate','--sync',window);time.sleep(1)
 command('caret.docEnd');x('windowactivate','--sync',window);x('key','--clearmodifiers','--delay','90','z','h','o','n','g','w','e','n');time.sleep(1.5)
 report['duringText']=call('document.inspect')['text'][-160:];report['activeWindow']=x('getactivewindow');capture('01-pinyin-candidate.png',True)
 x('windowactivate','--sync',window);x('key','--clearmodifiers','1');time.sleep(1)
 after=call('document.inspect')['text'];report['afterText']=after[-160:];capture('02-after-selection.png')
 report['nativeImeVerified']='中文' in after and 'zhongwen' not in after
 report['passed']=report['nativeImeVerified'];report['probeCompleted']=True
except Exception as e:report['error']=str(e);raise
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=5)
 (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
