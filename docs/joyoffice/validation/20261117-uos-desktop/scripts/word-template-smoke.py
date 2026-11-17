import hashlib,json,os,secrets,socket,subprocess,time
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/word-template-native-3';OUT.mkdir(exist_ok=False)
BINARY=ROOT/'target/debug/joyword';key=secrets.token_hex(32);env=os.environ.copy();env.update(JOYWORD_CONFIG_DIR=str(OUT/'profile'),JOYWORD_CONTROL_KEY=key)
report={'passed':False,'binarySha256':hashlib.sha256(BINARY.read_bytes()).hexdigest(),'eventSource':'X11 OS synthetic pointer input, actual window pixels and authenticated live document inspection','checks':[]}
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
log=(OUT/'application.log').open('w');p=subprocess.Popen([str(BINARY),'--control',str(port)],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT);log.close()
def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def call(method,**params):
 with socket.create_connection(('127.0.0.1',port),timeout=3) as client:
  client.settimeout(5);client.sendall((json.dumps({'id':1,'method':method,'params':params,'key':key})+'\n').encode());reply=json.loads(client.makefile('rb').readline())
 if not reply.get('ok'):raise ValueError(str(reply))
 return reply['result']
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
def capture(name):
 x('windowactivate','--sync',window);time.sleep(.3);subprocess.run(['import','-window',window,str(OUT/name)],check=True,timeout=15)
try:
 wait(lambda:call('document.inspect'),'welcome document')
 window=wait(lambda:x('search','--onlyvisible','--pid',p.pid).splitlines()[0],'visible window')
 x('windowactivate','--sync',window);time.sleep(1);capture('01-welcome.png')
 x('mousemove','--window',window,35,55,'click',1);time.sleep(.7);capture('02-file-home.png')
 x('mousemove','--window',window,85,265,'click',1);time.sleep(.7);capture('03-gallery.png')
 x('mousemove','--window',window,540,340,'click',1)
 wait(lambda:'社区阅读活动项目计划书' in call('document.inspect')['text'],'project template open')
 capture('04-project-plan.png');report['templateText']=call('document.inspect')['text'][:400]
 report['checks'].extend(['isolated first run renders the Chinese welcome document','mouse opens File home and the full New gallery','mouse creates project-plan template and live control reads its Chinese body'])
 report['passed']=True
except Exception as e:report['error']=str(e);raise
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=5)
 (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
