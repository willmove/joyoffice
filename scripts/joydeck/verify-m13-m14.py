"""Real macOS M13/M14 window acceptance, pixel checks and MP4 verification."""
import argparse,hashlib,importlib.util,json,os,subprocess,time
from pathlib import Path
from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
spec=importlib.util.spec_from_file_location('bench',SCRIPTS / 'benchmark-m4.py');bench=importlib.util.module_from_spec(spec);spec.loader.exec_module(bench)
def main():
 p=argparse.ArgumentParser();p.add_argument('--native-only',action='store_true');p.add_argument('--binary',type=Path,default=binary('joydeck', 'release'));p.add_argument('--output',type=Path,default=EVIDENCE / 'm14'/f'run-{time.time_ns()}');a=p.parse_args()
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);seen=set();rss=[];quality=None;foreground={}
 with (out/'native.log').open('wb') as log:
  proc=subprocess.Popen([str(a.binary.resolve()),'--beta-smoke'],cwd=ROOT,env=dict(os.environ,JOYDECK_M14_OUTPUT=str(out),JOYDECK_STATE_DIR=str(out/'state')),stdout=log,stderr=subprocess.STDOUT)
  try:
   deadline=time.monotonic()+150
   while time.monotonic()<deadline:
    logtext=(out/'native.log').read_text(errors='replace', encoding='utf-8')
    if 'failed=' in logtext or 'panicked' in logtext or 'startup/runtime failure' in logtext:raise RuntimeError(logtext[-3500:])
    value=subprocess.run(['ps','-o','rss=','-p',str(proc.pid)],capture_output=True,text=True, encoding='utf-8').stdout.strip()
    if value.isdigit():rss.append(int(value))
    for line in logtext.splitlines():
     if not line.startswith('M14_CAPTURE '):continue
     phase=int(line.split()[1])
     if phase in seen:continue
     if not a.native_only:
      foreground[str(phase)]=bench.focus_window(proc.pid)
      windows=bench.windows(proc.pid);assert len(windows)==1,windows
      subprocess.run(['screencapture','-x','-o','-l',str(windows[0]['kCGWindowNumber']),str(out/f'panel-{phase}.png')],check=True)
     if phase==2 and not a.native_only:
      bounds=json.loads(next(s.split(' ',1)[1] for s in logtext.splitlines() if s.startswith('M14_CANVAS ')))
      quality=bench.capture(proc.pid,out,'M14-compare',bounds);assert quality['passed'],quality
     seen.add(phase);(out/f'capture-{phase}.ack').touch()
    if proc.poll() is not None:
     assert proc.returncode==0 and 'M14_SMOKE complete=true' in logtext,logtext[-3500:]
     break
    time.sleep(.1)
   else:raise TimeoutError('native beta acceptance timeout')
  finally:
   if proc.poll() is None:proc.terminate();proc.wait(timeout=5)
 assert seen==set(range(5))
 state=json.loads((out/'native-state.json').read_text(encoding='utf-8'))
 prefs=json.loads((out/'state/preferences.json').read_text(encoding='utf-8'));assert prefs['zoom']==.9 and prefs['notes'] and len(prefs['recent'])==1
 # Decode every frame with a separate AVAssetReader, not the editor's AVPlayer.
 executable=out/'inspect-video'
 subprocess.run(['swiftc','-O',str(SCRIPTS / 'inspect-video.swift'),'-o',str(executable)],check=True,stdout=subprocess.DEVNULL,stderr=(out/'swift-build.log').open('w'))
 info=json.loads(subprocess.check_output([str(executable),str(out/'slides.mp4'),str(out/'first-frame.bgra')],text=True, encoding='utf-8'))
 assert info['frames']==270 and info['audioTracks']==0 and abs(info['duration']-18)<.05,info
 assert info['width']==960 and info['height']==540 and info['monotonicPts'],info
 from PIL import Image
 import numpy as np
 decoded=Image.frombytes('RGBA',(960,540),(out/'first-frame.bgra').read_bytes(),'raw','BGRA').convert('RGB')
 reference=Image.open(out/'M14-compare-reference.png').convert('RGB').resize(decoded.size,Image.Resampling.BILINEAR)
 error=float(np.abs(np.asarray(decoded,dtype=np.int16)-np.asarray(reference,dtype=np.int16)).mean());assert error<6,error
 decoded.save(out/'decoded-first-frame.png')
 cli=a.binary.resolve().parent/'joydeck-cli'
 def command(*args):
  return json.loads(subprocess.check_output([str(cli),'--read-root',str(out),'--write-root',str(out),*args],text=True, encoding='utf-8'))
 diff=command('compare',str(out/'current.joydeck'),str(out/'reference.joydeck'))['result']
 index=next(i for i,d in enumerate(diff['differences']) if d['kind']=='object.changed')
 commands=[{'method':'document.open','params':{'path':str(out/'current.joydeck')}},{'method':'document.acceptObject','params':{'path':str(out/'reference.joydeck'),'baseSha256':diff['base_sha256'],'referenceSha256':diff['reference_sha256'],'index':index}},{'method':'document.inspect','params':{}},{'method':'document.undo','params':{}},{'method':'document.inspect','params':{}}]
 result=command('run',json.dumps(commands,ensure_ascii=False))
 assert result[2]['result']['deck']['slides'][0]['objects'][0]['text']=='已核对的业务标题'
 assert result[4]['result']['deck']['slides'][0]['objects'][0]['text']!='已核对的业务标题'
 command('convert',str(out/'current.joydeck'),str(out/'titles.vtt'),'vtt')
 subtitles=(out/'titles.vtt').read_text(encoding='utf-8');assert subtitles.startswith('WEBVTT') and '00:00:18.000' in subtitles and '私密演讲者备注' not in subtitles
 diagnostics=command('diagnostics');assert set(diagnostics)=={'application','version','os','architecture','desktopFeature','nativeReaderMax','writerSchema','preferencesSchema','automationProtocol','videoExport','diagnosticScope'}
 assert rss and max(rss)<450*1024, max(rss,default=0)
 result={'binarySha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'cliSha256':hashlib.sha256(cli.read_bytes()).hexdigest(),'nativeOnly':a.native_only,'guiPassed':not a.native_only,'foregroundActivation':{'required':not a.native_only,'panels':foreground},'panelsCaptured':0 if a.native_only else len(seen),'canvas':quality,'nativeState':state,'video':info,'decodedFrameMeanRgbError':error,'cliCompareAcceptUndo':True,'titleVttTimeline':True,'diagnosticsFieldsChecked':True,'appMaxRssKiB':max(rss),'preferences':{'schema':prefs['schema'],'generation':prefs['generation'],'recentCount':len(prefs['recent'])},'passed':True}
 (out/'verification.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n', encoding='utf-8');print(json.dumps(result,ensure_ascii=False,indent=2));print(out)
if __name__=='__main__':main()
