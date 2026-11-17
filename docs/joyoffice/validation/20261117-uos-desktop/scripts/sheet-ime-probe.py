import hashlib,json,os,socket,subprocess,time
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/sheet-ime-native';OUT.mkdir(exist_ok=False);profile=OUT/'profile'
with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
env=os.environ.copy();env['JOYSHEET_CONFIG_DIR']=str(profile)
for name in ('JOYSHEET_CONTROL_TOKEN','GRIDCRAFT_CONTROL_TOKEN'):env.pop(name,None)
report={'passed':False,'probeCompleted':False,'nativeImeVerified':False,'inputMethod':'com.sogou.ime.ng.fcitx5.uos','eventSource':'X11 OS synthetic ASCII keys through active system input method; production control readback','binarySha256':hashlib.sha256((ROOT/'target/debug/joysheet').read_bytes()).hexdigest()}
log=(OUT/'application.log').open('w');p=subprocess.Popen([str(ROOT/'target/debug/joysheet'),'--sample','sales','--control',str(port),'--diagnostics'],cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def call(method,**params):
 token=(profile/f'control-{port}.token').read_text().strip()
 with socket.create_connection(('127.0.0.1',port),timeout=5) as client:
  stream=client.makefile('rw',encoding='utf-8')
  def exchange(value):
   stream.write(json.dumps(value,ensure_ascii=False)+'\n');stream.flush();reply=json.loads(stream.readline())
   if not reply.get('ok'):raise ValueError(str(reply))
   return reply['result']
  exchange({'id':0,'method':'auth','params':{'token':token}})
  return exchange({'id':1,'method':method,'params':params})
def wait(fn,label):
 deadline=time.monotonic()+30;last=None
 while time.monotonic()<deadline:
  if p.poll() is not None:raise RuntimeError(f'Sheet exited {p.returncode}: {label}')
  try:
   value=fn()
   if value:return value
  except (OSError,ValueError,subprocess.SubprocessError) as e:last=str(e)
  time.sleep(.1)
 raise TimeoutError(f'{label}: {last}')
def capture(name,crop=False):
 if crop:
  values={}
  for line in x('getwindowgeometry','--shell',window).splitlines():k,v=line.split('=',1);values[k]=v
  subprocess.run(['import','-window','root','-crop',f'{values["WIDTH"]}x{values["HEIGHT"]}+{values["X"]}+{values["Y"]}',str(OUT/name)],check=True,timeout=15)
 else:subprocess.run(['import','-window',window,str(OUT/name)],check=True,timeout=15)
try:
 subprocess.run(['fcitx5-remote','-o'],check=True);wait(lambda:call('ui.inspect')['perf']['frame']>1,'first frame')
 window=wait(lambda:x('search','--onlyvisible','--pid',p.pid).splitlines()[0],'visible window');call('ui.cell.click',cell='A1');x('windowactivate','--sync',window);time.sleep(1)
 x('key','--clearmodifiers','--delay','90','z','h','o','n','g','w','e','n');time.sleep(1.5)
 during=call('ui.inspect');report['duringEditor']=during['editor'];report['duringComposing']=during['composing'];capture('01-pinyin-candidate.png',True)
 x('windowactivate','--sync',window);x('key','--clearmodifiers','1');time.sleep(.7);report['afterSelectionEditor']=call('ui.inspect')['editor'];capture('02-after-selection.png')
 x('key','--clearmodifiers','Return');wait(lambda:call('ui.inspect')['editor'] is None,'cell commit');value=call('cell.get',cell='A1').get('value');report['cellValue']=value;capture('03-committed.png')
 report['nativeImeVerified']=value=='中文';report['passed']=report['nativeImeVerified'];report['probeCompleted']=True
except Exception as e:report['error']=str(e);raise
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=5)
 log.close();(OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
