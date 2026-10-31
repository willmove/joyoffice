"""M5 real macOS window/clipboard adapter and CPU/GPUI pixel validation."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('benchmark_m4',ROOT/'scripts/benchmark-m4.py')
bench=importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=ROOT/'target/debug/joydeck')
    parser.add_argument('--output',type=Path,default=ROOT/'docs/evidence/m5'/f'run-{time.time_ns()}')
    args=parser.parse_args(); output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    quality={};seen=set()
    with (output/'native.log').open('wb') as log:
        process=subprocess.Popen([str(args.binary.resolve()),'--media-smoke'],cwd=ROOT,env=dict(os.environ,JOYDECK_M5_OUTPUT=str(output)),stdout=log,stderr=subprocess.STDOUT)
        deadline=time.monotonic()+45
        try:
            while time.monotonic()<deadline:
                text=(output/'native.log').read_text(errors='replace')
                if 'M5_SMOKE failed=' in text or 'panicked at' in text or 'startup/runtime failure' in text: raise RuntimeError(text[-3000:])
                for line in text.splitlines():
                    if not line.startswith('M5_CAPTURE '):continue
                    _,backend,bounds=line.split(' ',2)
                    if backend in seen:continue
                    quality[backend]=bench.capture(process.pid,output,f'M5-{backend}',json.loads(bounds))
                    seen.add(backend)
                    (output/f'capture-{0 if backend=="GPUI" else 1}.ack').touch()
                if process.poll() is not None:
                    if process.returncode!=0 or 'M5_SMOKE complete=true' not in text: raise RuntimeError(text[-3000:])
                    break
                time.sleep(.1)
            else:raise RuntimeError('M5 real-window validation timeout')
        finally:
            if process.poll() is None:process.terminate();process.wait(timeout=5)
    result={'binary':str(args.binary.resolve()),'pixelChecks':quality,'passed':len(quality)==2 and all(q['passed'] for q in quality.values()),'scope':'macOS native adapter; physical keyboard, Windows/Linux unverified'}
    (output/'verification.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2));print(output)
    if not result['passed']:raise SystemExit(1)
if __name__=='__main__':main()
