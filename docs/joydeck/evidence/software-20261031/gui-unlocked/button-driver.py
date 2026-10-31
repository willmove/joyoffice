import hashlib,importlib.util,json,os,shutil,subprocess,sys,time,zipfile
from pathlib import Path
root=Path('/Users/willm/Coding/joyoffice');out=root/'dist/joydeck/software-gui-button-20261031'
package=root/'dist/joydeck/joydeck-software-m12-m14-20261031-delivery'
source=root/'dist/joydeck/software-gui-20261031-unlocked'
sys.path.insert(0,str(root/'scripts/joydeck'))
spec=importlib.util.spec_from_file_location('bench',root/'scripts/joydeck/benchmark-m4.py');bench=importlib.util.module_from_spec(spec);spec.loader.exec_module(bench)
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def patch(src,dst,image=False):
    with zipfile.ZipFile(src) as z: entries={n:z.read(n) for n in z.namelist()}
    deck=json.loads(entries['document.json']);deck['title']='图片依赖验收'
    if image:
        added=deck['slides'][0]['objects'][-1]
        assert added['kind']=='image'
        added.update(x=650,y=320,width=240,height=160)
    entries['document.json']=json.dumps(deck,ensure_ascii=False).encode()
    with zipfile.ZipFile(dst,'w',compression=zipfile.ZIP_DEFLATED) as z:
        for n,data in entries.items():z.writestr(n,data)
patch(source/'image-base.joydeck',out/'base.joydeck')
patch(source/'image-reference.joydeck',out/'reference.joydeck',True)
app=out/'JoyDeck UI Test.app'
if not app.exists():shutil.copytree(package/'JoyDeck.app',app)
state=out/'state';state.mkdir(exist_ok=True)
(state/'preferences.json').write_text(json.dumps({'schema':2,'generation':0,'recent':[],'zoom':.9,'inspector':False,'notes':True,'width':1600,'height':900}))
helper=out/'input-source';previous=subprocess.check_output([str(helper)],text=True).strip()
proc=None;wake=None;events=[];captures=[]
def wait(test,timeout=15):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        if proc and proc.poll() is not None:raise RuntimeError('App exited')
        if test():return
        time.sleep(.1)
    raise TimeoutError('UI condition timed out')
def applescript(body):
    return subprocess.check_output(['osascript','-e',f'tell application "System Events" to tell (first application process whose unix id is {proc.pid})\nset frontmost to true\n{body}\nend tell'],text=True)
def native():
    with zipfile.ZipFile(out/'base.joydeck') as z:
        model=json.loads(z.read('document.json')); resources=[n for n in z.namelist() if n.startswith('resources/')]
    return {'objects':len(model['slides'][0]['objects']),'resources':resources,'model':model}
def selected():
    wins=bench.windows(proc.pid)
    assert wins[0]['kCGWindowNumber']==window['kCGWindowNumber'],wins
    return wins[0]
def capture(name):
    activation=bench.focus_window(proc.pid);w=selected()
    file=out/(name+'.png')
    subprocess.run(['screencapture','-x','-o','-l',str(w['kCGWindowNumber']),str(file)],check=True)
    captures.append({'name':name,'sha256':digest(file),'foreground':activation})
    print(json.dumps({'capture':str(file),'native':{k:v for k,v in native().items() if k!='model'}},ensure_ascii=False),flush=True)
def click(x,y):
    bench.focus_window(proc.pid);w=selected();b=w['kCGWindowBounds']
    subprocess.run([str(out/'click'),str(b['X']+x),str(b['Y']+y)],check=True)
    events.append({'type':'mouse','x':x,'y':y});time.sleep(.3)
def save():
    applescript('keystroke "s" using command down');time.sleep(.7)
try:
    for candidate in ('com.apple.keylayout.ABC','com.apple.keylayout.US'):
        if subprocess.run([str(helper),candidate]).returncode==0:break
    else:raise RuntimeError('No enabled Latin input source')
    with (out/'native.log').open('wb') as log:
        proc=subprocess.Popen([str(app/'Contents/MacOS/JoyDeck'),'--integration-smoke','--table-demo'],cwd=out,env=dict(os.environ,JOYDECK_STATE_DIR=str(state)),stdout=log,stderr=subprocess.STDOUT)
        wake=subprocess.Popen(['caffeinate','-d','-i','-w',str(proc.pid)])
        wait(lambda:len(bench.windows(proc.pid))==1)
        subprocess.run(['open','-a',str(app),str(out/'base.joydeck')],check=True)
        wait(lambda:len(bench.windows(proc.pid))==2)
        time.sleep(1)
        window=next(w for w in bench.windows(proc.pid) if '未命名文稿' not in w.get('kCGWindowName',''))
        baseline=native();capture('01-baseline')
        print(json.dumps({'ready':True,'pid':proc.pid,'window':window,'baselineObjects':baseline['objects']},ensure_ascii=False),flush=True)
        for line in sys.stdin:
            a=json.loads(line)
            if a['action']=='click':click(a['x'],a['y'])
            elif a['action']=='type-reference':
                applescript('keystroke "a" using command down\nkeystroke '+json.dumps(str(out/'reference.joydeck')))
            elif a['action']=='capture':capture(a['name'])
            elif a['action']=='save':save()
            elif a['action']=='undo':applescript('keystroke "z" using command down');time.sleep(.4)
            elif a['action']=='redo':applescript('keystroke "z" using {command down,shift down}');time.sleep(.4)
            elif a['action']=='assert-adopted':
                wait(lambda:native()['objects']==baseline['objects']+1 and len(native()['resources'])==1)
                events.append({'type':'adopted-file','objects':native()['objects'],'resourceCount':1})
            elif a['action']=='assert-original':
                wait(lambda:native()['model']==baseline['model'] and native()['resources']==baseline['resources'])
                events.append({'type':'baseline-restored','objects':native()['objects'],'resourceCount':0})
            elif a['action']=='wait':time.sleep(a.get('seconds',1))
            elif a['action']=='finish':break
            else:raise ValueError(a)
            print('ACTION_OK '+a['action'],flush=True)
        result={'binarySha256':digest(app/'Contents/MacOS/JoyDeck'),'events':events,'captures':captures,'passed':any(e['type']=='adopted-file' for e in events) and any(e['type']=='baseline-restored' for e in events),'physicalMouse':False,'eventSource':'CoreGraphics synthetic mouse; System Events keyboard'}
        (out/'verification.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
        print(json.dumps(result,ensure_ascii=False,indent=2),flush=True)
finally:
    restored=subprocess.run([str(helper),previous]).returncode==0
    if proc and proc.poll() is None:proc.terminate();proc.wait(timeout=8)
    if wake:
        if wake.poll() is None:wake.terminate()
        wake.wait(timeout=5)
    subprocess.run(['/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister','-u',str(app)],capture_output=True)
    if not restored:raise RuntimeError('Input source restore failed')
