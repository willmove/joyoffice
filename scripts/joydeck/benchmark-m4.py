"""Measure fixed native CPU/GPUI scenes and compare actual canvas pixels with CPU references.
Use the bundled Python runtime (Pillow/numpy) for this QA tool, on an unlocked macOS desktop.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import time
import numpy as np
from PIL import Image, ImageCms

from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
from foreground import focus_window

def srgb_image(path):
    image=Image.open(path).convert('RGB')
    profile=image.info.get('icc_profile')
    if profile:
        image=ImageCms.profileToProfile(image,ImageCms.ImageCmsProfile(io.BytesIO(profile)),ImageCms.createProfile('sRGB'),outputMode='RGB')
    return image

def windows(pid):
    js=f'ObjC.import("CoreGraphics"); var list=ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo(1,0))); JSON.stringify(list.filter(x=>x.kCGWindowOwnerPID==={pid}&&x.kCGWindowLayer===0));'
    return json.loads(subprocess.check_output(['osascript','-l','JavaScript','-e',js],text=True))

def capture(pid, output, label, bounds):
    foreground=focus_window(pid)
    entries=windows(pid)
    if len(entries)!=1: raise RuntimeError('Expected one native benchmark window')
    window=entries[0]
    path=output/f'{label}-window.png'
    subprocess.run(['screencapture','-x','-o','-l',str(window['kCGWindowNumber']),str(path)],check=True)
    image=srgb_image(path)
    frame=window['kCGWindowBounds']
    dpi=image.width/frame['Width']
    title=frame['Height']-bounds['viewportHeight']
    rect=tuple(round(v*dpi) for v in (bounds['x'],bounds['y']+title,bounds['x']+bounds['width'],bounds['y']+title+bounds['height']))
    crop=image.crop(rect)
    reference=Image.open(output/f'{label}-reference.png').convert('RGB')
    if crop.size!=reference.size: crop=crop.resize(reference.size,Image.Resampling.BOX)
    crop.save(output/f'{label}-canvas.png')
    delta=np.abs(np.asarray(crop,dtype=np.int16)-np.asarray(reference,dtype=np.int16))
    result={'foregroundActivation':foreground,'meanRgbError':float(delta.mean()),'fractionPixelsOver48':float((delta.max(axis=2)>48).mean())}
    # Distinguish backend AA edges from incorrect flat colors/alpha/order.
    a=np.asarray(crop,dtype=np.int16);b=np.asarray(reference,dtype=np.int16);h,w=a.shape[:2]
    padded=np.pad(b,((1,1),(1,1),(0,0)),mode='edge')
    neighbors=np.stack([padded[y:y+h,x:x+w] for y in range(3) for x in range(3)])
    interior=(neighbors.max(axis=0)-neighbors.min(axis=0)).max(axis=2)<=8
    result['interiorRgbError']=float(delta[interior].mean()) if interior.any() else 0.
    result['interiorPixelsOver16']=float((delta.max(axis=2)[interior]>16).mean()) if interior.any() else 0.
    result['edgePixelsOver128']=float(((delta.max(axis=2)>128)&~interior).mean())
    result['passed']=result['meanRgbError']<=5. and result['interiorRgbError']<=1. and result['interiorPixelsOver16']<=.001 and result['edgePixelsOver128']<=.002
    Image.fromarray(np.minimum(delta*4,255).astype(np.uint8)).save(output/f'{label}-difference.png')
    return result

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=binary('joydeck', 'release'))
    parser.add_argument('--output',type=Path,default=EVIDENCE / 'm4'/f'run-{time.time_ns()}')
    parser.add_argument('--diagnostic',action='store_true',help='Record debug/performance failures without accepting M4')
    args=parser.parse_args();output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    env=dict(os.environ,JOYDECK_M4_OUTPUT=str(output),JOYDECK_M4_CAPTURE_ACK='1')
    log=output/'native.log';quality={};processed=set();max_rss_kib=0;last_rss_sample=0.
    with log.open('wb')as stream:
        process=subprocess.Popen([str(args.binary.resolve()),'--m4-bench','--hold'],cwd=ROOT,env=env,stdout=stream,stderr=subprocess.STDOUT)
        deadline=time.monotonic()+180
        try:
            while time.monotonic()<deadline:
                if time.monotonic()-last_rss_sample>=.5:
                    sample=subprocess.run(['ps','-o','rss=','-p',str(process.pid)],text=True,capture_output=True)
                    if sample.stdout.strip().isdigit():max_rss_kib=max(max_rss_kib,int(sample.stdout.strip()))
                    last_rss_sample=time.monotonic()
                text=log.read_text(errors='replace')
                if 'startup/runtime failure' in text: raise RuntimeError(f'Native benchmark panic: {log}')
                for line in text.splitlines():
                    if not line.startswith('M4_CAPTURE '): continue
                    _,scenario,backend,raw=line.split(' ',3);label=f'{scenario}-{backend}'
                    if label in processed: continue
                    # CPU references are produced after the post-render marker.
                    if not (output/f'{label}-reference.png').exists():continue
                    quality[label]=capture(process.pid,output,label,json.loads(raw))
                    processed.add(label);(output/f'capture-{len(processed)-1}.ack').touch()
                if 'M4_BENCH complete=true' in text:
                    report=json.loads((output/'native-benchmark.json').read_text())
                    report['binarySha256']=hashlib.sha256(args.binary.resolve().read_bytes()).hexdigest()
                    report['rssKiBMaxObserved']=max_rss_kib
                    report['rssSamplingIntervalSeconds']=.5
                    report['pixelQuality']=quality
                    report['pixelThresholds']={'meanRgbError':5.,'interiorRgbError':1.,'interiorPixelsOver16':.001,'edgePixelsOver128':.002,'stableNeighborhoodColorRange':8,'edgeNeighborhoodRadius':1}
                    report['releaseRenderingValidation']=report['profile']=='release' and report['measuredRenderingGate'] and len(quality)==4 and all(v['passed']for v in quality.values())
                    report['rssBudgetMiB']=450
                    report['memoryValidation']=max_rss_kib>0 and max_rss_kib<=450*1024
                    report['releaseValidation']=report['releaseRenderingValidation'] and report['memoryValidation']
                    (output/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
                    print(json.dumps({'output':str(output),'releaseValidation':report['releaseValidation'],'rssMiBObserved':max_rss_kib/1024,'releaseRenderingValidation':report['releaseRenderingValidation'],'relativeP95Reduction':report['editRelativeP95Reduction'],'pixels':quality},ensure_ascii=False),flush=True)
                    if not args.diagnostic and not report['releaseValidation']: raise RuntimeError('Rendering/performance validation failed; inspect verification.json')
                    return
                if process.poll()is not None:raise RuntimeError(f'Application exited {process.returncode}: {log}')
                time.sleep(.05)
            raise TimeoutError(f'Benchmark timed out: {log}')
        finally:
            if process.poll()is None:process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()

if __name__=='__main__':main()
