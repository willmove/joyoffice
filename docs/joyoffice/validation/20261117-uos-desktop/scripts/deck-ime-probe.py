import json,os,select,shutil,subprocess,time
from pathlib import Path
ROOT=Path.cwd();OUT=ROOT/'target/uos-validation-20261117/deck-ime-native';OUT.mkdir(exist_ok=False)
SOURCE=OUT/'ime.pptx';shutil.copyfile(ROOT/'tests/fixtures/joydeck/r1-preview.pptx',SOURCE)
env=os.environ.copy();env['XDG_STATE_HOME']=str(OUT/'state');env.pop('LOCALAPPDATA',None)
report={'passed':False,'probeCompleted':False,'nativeImeVerified':False,'inputMethod':'com.sogou.ime.ng.fcitx5.uos','eventSource':'X11 OS synthetic ASCII keys through active system input method; desktop stdio readback'}
log=(OUT/'application.log').open('w');p=subprocess.Popen([str(ROOT/'target/debug/joydeck'),str(SOURCE),f'--control-stdio={OUT}'],cwd=ROOT,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True,encoding='utf-8',bufsize=1);log.close();request_id=0
def x(*args):return subprocess.check_output(['xdotool',*map(str,args)],text=True,timeout=10).strip()
def call(method,params=None,mutating=False):
 global request_id
 status=call('document.status') if mutating or method=='document.inspect' else None
 request_id+=1;r={'protocolVersion':'joydeck-control-1','requestId':f'ime-{request_id}','method':method,'params':params or {}}
 if status:r.update(target=status['documentId'],expectedRevision=status.get('revision'))
 p.stdin.write(json.dumps(r,ensure_ascii=False)+'\n');p.stdin.flush()
 if not select.select([p.stdout],[],[],10)[0]:raise TimeoutError('stdio response')
 response=json.loads(p.stdout.readline())
 if 'error' in response:raise ValueError(str(response['error']))
 return response['result']
def wait(fn,label):
 deadline=time.monotonic()+30;last=None
 while time.monotonic()<deadline:
  if p.poll() is not None:raise RuntimeError(f'Deck exited {p.returncode}: {label}')
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
 subprocess.run(['fcitx5-remote','-o'],check=True);wait(lambda:call('document.status').get('pages'),'document load')
 window=wait(lambda:x('search','--onlyvisible','--pid',p.pid).splitlines()[0],'visible window');x('windowactivate','--sync',window);time.sleep(1)
 x('mousemove','--window',window,620,1050,'click',1);x('key','--clearmodifiers','--delay','90','z','h','o','n','g','w','e','n');time.sleep(1.5)
 during=json.dumps(call('document.inspect'),ensure_ascii=False);report['duringContainsPinyin']='zhongwen' in during;capture('01-pinyin-candidate.png',True)
 x('windowactivate','--sync',window);x('key','--clearmodifiers','1');time.sleep(1)
 after=json.dumps(call('document.inspect'),ensure_ascii=False);report['afterContainsPinyin']='zhongwen' in after;report['afterContainsChinese']='中文' in after;capture('02-after-selection.png')
 report['nativeImeVerified']=report['afterContainsChinese'] and not report['afterContainsPinyin'];report['passed']=report['nativeImeVerified'];report['probeCompleted']=True
except Exception as e:report['error']=str(e);raise
finally:
 if p.poll() is None:p.terminate();p.wait(timeout=5)
 (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False))
