"""M6 real macOS window/clipboard adapter and CPU/GPUI pixel validation."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
spec=importlib.util.spec_from_file_location('benchmark_m4',SCRIPTS / 'benchmark-m4.py')
bench=importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)
all_windows=bench.windows
bench.windows=lambda pid:[w for w in all_windows(pid) if w.get('kCGWindowName')=='JoyDeck — M6 Acceptance Source']

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=binary('joydeck', 'debug'))
    parser.add_argument('--output',type=Path,default=EVIDENCE / 'm6'/f'run-{time.time_ns()}')
    args=parser.parse_args(); output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    quality={};seen=set();panel=False
    with (output/'native.log').open('wb') as log:
        process=subprocess.Popen([str(args.binary.resolve()),'--workspace-smoke'],cwd=ROOT,env=dict(os.environ,JOYDECK_M6_OUTPUT=str(output)),stdout=log,stderr=subprocess.STDOUT)
        deadline=time.monotonic()+45
        try:
            while time.monotonic()<deadline:
                text=(output/'native.log').read_text(errors='replace')
                if 'M6_SMOKE failed=' in text or 'panicked at' in text or 'startup/runtime failure' in text: raise RuntimeError(text[-3000:])
                for line in text.splitlines():
                    if not line.startswith('M6_CAPTURE '):continue
                    _,backend,bounds=line.split(' ',2)
                    if backend in seen:continue
                    quality[backend]=bench.capture(process.pid,output,f'M6-{backend}',json.loads(bounds))
                    seen.add(backend)
                    (output/f'capture-{0 if backend=="GPUI" else 1}.ack').touch()
                if 'M6_PANEL_READY' in text and not panel:
                    window=bench.windows(process.pid)
                    if len(window)!=1:raise RuntimeError('Expected one source acceptance window')
                    subprocess.run(['screencapture','-x','-o','-l',str(window[0]['kCGWindowNumber']),str(output/'M6-workspace-panel.png')],check=True)
                    panel=True
                    (output/'panel.ack').touch()
                if process.poll() is not None:
                    if process.returncode!=0 or 'M6_SMOKE complete=true' not in text: raise RuntimeError(text[-3000:])
                    break
                time.sleep(.1)
            else:raise RuntimeError('M6 real-window validation timeout')
        finally:
            if process.poll() is None:process.terminate();process.wait(timeout=5)
    result={'binary':str(args.binary.resolve()),'pixelChecks':quality,'passed':len(quality)==2 and all(q['passed'] for q in quality.values()),'scope':'macOS native adapter; physical keyboard, Windows/Linux unverified'}
    (output/'verification.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2));print(output)
    if not result['passed']:raise SystemExit(1)
if __name__=='__main__':main()
