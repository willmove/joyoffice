from pathlib import Path
import argparse,subprocess,json,time,queue,threading,statistics,hashlib,sys
ap=argparse.ArgumentParser();ap.add_argument('--cli',type=Path,required=True);ap.add_argument('--prefix',required=True);args=ap.parse_args()
root=Path('target/joydeck-dpi-check').resolve();exe=args.cli.resolve()
sys.path.insert(0,str(Path('target/corpus-tools').resolve()))
from PIL import Image
p=subprocess.Popen([str(exe),'mcp','--root',str(root)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8');q=queue.Queue()
def reader():
 for line in p.stdout:q.put(line)
threading.Thread(target=reader,daemon=True).start();seq=0;target=None;revision=0;records=[]
def rpc(method,params={}):
 global seq
 seq+=1;p.stdin.write(json.dumps(dict(jsonrpc='2.0',id=seq,method=method,params=params))+'\n');p.stdin.flush();r=json.loads(q.get(timeout=120));assert r['id']==seq,r;return r
rpc('initialize',dict(protocolVersion='2025-11-25',capabilities={},clientInfo=dict(name='dpi-qa',version='1')))
p.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');p.stdin.flush()
def call(method,params={}):
 r=rpc('tools/call',dict(name='joydeck.control',arguments=dict(protocolVersion='joydeck-control-1',requestId=f'dpi-{seq}',method=method,params=params,target=target,expectedRevision=revision)))['result']['structuredContent'];assert 'error' not in r,r;return r['result']
try:
 opened=call('document.open',dict(path=str(root/'clarity.pptx')));target=opened['documentId'];call('show.start')
 for kind in ['static','transition']:
  if kind=='transition':call('show.action',dict(action='goto',page=2));call('show.tick',dict(seconds=.5))
  for scale in [1,2,4]:
   for repetition in range(3):
    path=root/f'{args.prefix}-{kind}-{scale}-{repetition}.png';start=time.perf_counter();call('show.frame',dict(path=str(path),scale=scale));ms=(time.perf_counter()-start)*1000
    with Image.open(path) as image:dimensions=image.size
    assert dimensions==(960*scale,540*scale),dimensions
    records.append(dict(kind=kind,scale=scale,repetition=repetition,ms=round(ms,2),dimensions=dimensions))
finally:p.stdin.close();p.wait(timeout=30)
medians=[dict(kind=kind,scale=scale,ms=round(statistics.median(r['ms'] for r in records if r['kind']==kind and r['scale']==scale),2)) for kind in ['static','transition'] for scale in [1,2,4]]
result=dict(note='Idle-host sequential runs; median of 3 end-to-end RPC + raster + PNG encode/write calls; not display FPS',executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),fixture_sha256=hashlib.sha256((root/'clarity.pptx').read_bytes()).hexdigest(),records=records,medians=medians)
(root/f'{args.prefix}-timings.json').write_bytes((json.dumps(result,indent=2)+'\n').encode());print(args.prefix,json.dumps(medians))
