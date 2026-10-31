"""Real AVFoundation audio clock run; does not claim acoustic or A/V device calibration."""
import argparse,json,subprocess,tempfile,time,os,hashlib
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--seconds',type=float,default=600);a=p.parse_args()
from paths import ROOT as root, FIXTURES, EVIDENCE, native_helper
helper_hash=hashlib.sha256((native_helper('media')).read_bytes()).hexdigest()
with tempfile.TemporaryDirectory(prefix='joydeck-media-bench-') as temp:
 proc=subprocess.Popen([str(native_helper('media')),'play',str(FIXTURES / 'media/original-ten-minutes.wav'),temp,'0','600','0.05','false','true'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True, encoding='utf-8')
 start=None;first=None;drifts=[];last={};max_rss=0
 try:
  for line in proc.stdout:
   v=json.loads(line);assert 'error' not in v,v
   if v['playing']:
    if start is None:start=time.monotonic();first=v['seconds']
    elapsed=time.monotonic()-start;drift=(v['seconds']-first)-elapsed;drifts.append(drift);last=v
    if len(drifts)%300==0:
     rss=subprocess.check_output(['ps','-o','rss=','-p',str(proc.pid)],text=True, encoding='utf-8').strip();max_rss=max(max_rss,int(rss));print(json.dumps({'elapsed':round(elapsed,1),'clockDriftMs':round(drift*1000,2),'rssKiB':rss}),flush=True)
    if elapsed>=a.seconds-0.2:break
   if v.get('ended'):break
 finally:
  proc.terminate();proc.wait(timeout=5)
 elapsed=time.monotonic()-start if start else 0
 result={'helperSha256':helper_hash,'requestedSeconds':a.seconds,'elapsedSeconds':elapsed,'samples':len(drifts),'maxAbsoluteClockDriftMs':max(abs(x) for x in drifts)*1000 if drifts else None,'last':last,'adapterMaxRssKiB':max_rss,'scope':'AVPlayer position vs monotonic host; low volume actual default device output; not acoustic measurement or A/V sync calibration','passed':elapsed>=a.seconds-0.5 and max(abs(x) for x in drifts)<0.15}
 (EVIDENCE / 'm11').mkdir(parents=True,exist_ok=True)
 (EVIDENCE / 'm11/audio-clock-bench.json').write_text(json.dumps(result,indent=2)+'\n', encoding='utf-8');print(json.dumps(result),flush=True)
 if not result['passed']:raise SystemExit(1)
