"""Exercise the actual executable; independent OOXML/image/PDF metadata is checked separately."""
from pathlib import Path
import argparse,hashlib,json,queue,shutil,subprocess,sys,tempfile,threading,time,zipfile
ROOT=Path(__file__).resolve().parents[2]

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--cli',type=Path,default=ROOT/'target/debug/joydeck-cli.exe');ap.add_argument('--evidence',type=Path,default=ROOT/'docs/joydeck/evidence/R6-R7/cli-verification.json');args=ap.parse_args()
    sys.path.insert(0,str(ROOT/'target/corpus-tools'))
    from pptx import Presentation
    from pptx.package import Package
    from PIL import Image
    exe=args.cli.resolve();work=Path(tempfile.mkdtemp(prefix='joydeck-r6-r7-'))/'中文 空格';work.mkdir();source=ROOT/'tests/fixtures/joydeck/r6-r7';records=[];protocol=[]
    hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in source.iterdir() if p.is_file()}
    def run(values,expected=0):
        r=subprocess.run([str(exe),*map(str,values)],capture_output=True,encoding='utf-8',timeout=150);records.append(dict(args=list(map(str,values)),exit_code=r.returncode,stdout=r.stdout,stderr=r.stderr));assert r.returncode==expected,(values,r.stderr);return r.stdout
    public=source/'delivery-media.pptx';info=json.loads(run(['inspect',public]));assert len(info['slides'])==3
    for old,count in [('legacy-1.0',20),('legacy-1.11',1)]:
        original=source/(old+'.joydeck');dest=work/(old+'.joydeck');run(['migrate',original,'--output',dest]);back=json.loads(run(['inspect',dest]));assert len(back['slides'])==count and any('One-time' in s for s in back['warnings'])
        with zipfile.ZipFile(dest) as z:
            assert z.read('migration/legacy-original.joydeck')==original.read_bytes()
            assert json.loads(z.read('migration/report.json'))['source_ids']
        pp=work/(old+'.pptx');run(['exchange',dest,'--mode','preserve','--output',pp]);assert len(Presentation(pp).slides)==count
        clone=work/(old+'-original.joydeck');shutil.copy2(original,clone);run(['migrate',clone,'--output',clone],1);assert clone.read_bytes()==original.read_bytes()
    def patched(name,change):
        dest=work/(name+'.joydeck')
        with zipfile.ZipFile(source/'legacy-1.11.joydeck') as a,zipfile.ZipFile(dest,'w',zipfile.ZIP_DEFLATED) as b:
            for n in a.namelist():
                data=a.read(n)
                if n=='manifest.json':m=json.loads(data);change(m);data=json.dumps(m).encode()
                b.writestr(n,data)
        return dest
    for name,change in [('future',lambda m:m.update(schemaMinor=99)),('required',lambda m:m.update(requiredFeatures=['future-required-v99'])),('unknown-field',lambda m:m.update(futureField=True))]:
        bad=patched(name,change);dest=work/(name+'-must-not-exist.joydeck');run(['migrate',bad,'--output',dest],1);assert not dest.exists()
    for mode,ext in [('preserve','pptx'),('regenerate','pptx'),('regenerate','potx'),('regenerate','ppsx')]:
        dest=work/f'{mode}.{ext}';report=json.loads(run(['exchange',public,'--mode',mode,'--output',dest]));assert report['sourceUnchanged'];assert len(Package.open(str(dest)).main_document_part.presentation.slides)==3
        with zipfile.ZipFile(dest) as z:
            if mode=='preserve':assert z.read('vendor/retain-r6.xml')==b'<original>exact source payload</original>'
            else:assert 'vendor/retain-r6.xml' not in z.namelist() and any('Explicit regeneration' in s for s in report['warnings'])
    for fmt,layout in [('pdf','slides'),('pdf','notes'),('pdf','handouts-3'),('png','slides'),('jpeg','slides'),('gif','slides'),('mp4','slides'),('outline','slides'),('rtf','slides'),('vtt','slides')]:
        dest=work/(f'{fmt}-{layout}'+('' if fmt in ['png','jpeg'] else '.'+fmt));values=['export',public,'--format',fmt,'--output',dest,'--layout',layout]
        if fmt in ['gif','mp4']:values+=['--seconds','.5','--fps','4','--scale','.5']
        if fmt=='png':values+=['--transparent']
        report=json.loads(run(values));assert report['pages']==[1,2,3] and report['bytes']>0
        if fmt in ['png','jpeg']:
            files=sorted(dest.iterdir());assert len(files)==3
            for f in files:
                with Image.open(f) as im:assert im.size==(720,405);assert abs(im.info['dpi'][0]-72)<1
        if fmt=='gif':
            with Image.open(dest) as im:assert im.n_frames>=3
        if fmt=='vtt':assert 'PRIVATE SPEAKER' not in dest.read_text() and dest.read_text().startswith('WEBVTT')
    for values in [['--pages','0'],['--dpi','301'],['--scale','nan'],['--format','jpeg','--transparent']]:
        dest=work/f'invalid-{len(records)}';run(['export',public,'--output',dest,*values],1);assert not dest.exists()
    existing=work/'existing.pdf';existing.write_bytes(b'keep');run(['export',public,'--output',existing],1);assert existing.read_bytes()==b'keep'
    probe=json.loads(run(['media-inspect',source/'original-h264-aac.mp4']));assert probe['ready'] and probe['video']
    # Real MCP transport and JSON control use the same revision/replay dispatcher.
    local=work/'input.pptx';shutil.copyfile(public,local)
    proc=subprocess.Popen([str(exe),'mcp','--root',str(work)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8');inbox=queue.Queue()
    def reader():
        for line in proc.stdout:inbox.put(line)
    threading.Thread(target=reader,daemon=True).start();seq=0;target=None;revision=0
    def rpc(method,params=None):
        nonlocal seq
        seq+=1;r=dict(jsonrpc='2.0',id=seq,method=method,params=params or {});proc.stdin.write(json.dumps(r,ensure_ascii=False)+'\n');proc.stdin.flush();reply=json.loads(inbox.get(timeout=60));assert reply['id']==seq;protocol.append([r,reply]);return reply
    def call(method,params=None,rev=None,rid=None):
        request=dict(protocolVersion='joydeck-control-1',requestId=rid or f'r-{seq}',method=method,params=params or {},target=target,expectedRevision=revision if rev is None else rev)
        r=rpc('tools/call',dict(name='joydeck.control',arguments=request));return r['result']['structuredContent']
    try:
        assert rpc('tools/list')['error']['code']==-32002
        assert rpc('initialize',dict(protocolVersion='2025-11-25',capabilities={},clientInfo=dict(name='qa',version='1')))['result']['protocolVersion']=='2025-11-25'
        proc.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');proc.stdin.flush();assert len(rpc('tools/list')['result']['tools'])==1
        opened=call('document.open',dict(path=str(local)))['result'];target=opened['documentId'];assert 0<target<2**53
        selected=call('command.execute',dict(command=dict(command='select',page=1,shapes=[])),rid='selection');assert 'result' in selected;revision=selected['result']['revision'];assert revision==1
        assert call('command.execute',dict(command=dict(command='insert-text',text='stale',x=20,y=20)),rev=0)['error']
        params=dict(command=dict(command='insert-text',text='Control 中文 😀',x=20,y=20));edited=call('command.execute',params,rid='insert');assert 'result' in edited,edited
        assert call('command.execute',params,rid='insert')==edited
        revision=edited['result']['revision'];assert revision==2
        assert call('command.execute',dict(command=dict(command='undo')),rid='insert')['error']['code']=='id-conflict'
        inspected=call('document.inspect')['result'];assert sum(o.get('text')=='Control 中文 😀' for o in inspected['slides'][0]['objects'])==1
        assert call('document.open',dict(path=str(Path.home()/'outside.pptx')))['error']
        saved=call('document.save',dict(path=str(work/'control.pptx')))['result'];revision=saved['revision'];assert not saved['dirty']
        assert call('document.exchange',dict(path=str(work/'control-regenerated.pptx'),mode='regenerate'))['result']['sourceUnchanged']
        job=call('export.start',dict(path=str(work/'control.pdf'),options=dict(format='pdf',pages=[0])))['result']['jobId']
        for _ in range(150):
            state=call('jobs.get',dict(jobId=job))['result']
            if state['state']!='running':break
            time.sleep(.03)
        assert state['state']=='completed',state
        show=call('show.start')['result'];assert show['order']==[1,3]
        assert call('show.action',dict(action='next'))['result']['status']['page']==3
        call('show.frame',dict(path=str(work/'show.png'),scale=.5));call('show.action',dict(action='pause'));a=call('show.status')['result'];call('show.tick',dict(seconds=2));b=call('show.status')['result'];assert a['total_seconds']==b['total_seconds']
        call('show.action',dict(action='black'));call('show.frame',dict(path=str(work/'black.png'),scale=.25))
        with Image.open(work/'black.png') as im:assert im.convert('RGB').getextrema()==((0,0),(0,0),(0,0))
        # Export cancellation has deterministic pre-publication refusal; the worker is joined on EOF.
        job=call('export.start',dict(path=str(work/'cancel.gif'),options=dict(format='gif',pages=[0],seconds=15,fps=10,scale=.5)))['result']['jobId'];call('jobs.cancel',dict(jobId=job))
        for _ in range(150):
            state=call('jobs.get',dict(jobId=job))['result']
            if state['state']!='running':break
            time.sleep(.03)
        assert state['state']=='cancelled' and not (work/'cancel.gif').exists(),state
    finally:
        proc.stdin.close();proc.wait(timeout=30);assert proc.returncode==0,proc.stderr.read()
    for p in source.iterdir():
        if p.is_file():assert hashlib.sha256(p.read_bytes()).hexdigest()==hashes[p.name]
    result=dict(cli=str(exe),cli_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),work=str(work),fixture_hashes=hashes,records=records,protocol=protocol)
    args.evidence.parent.mkdir(parents=True,exist_ok=True);args.evidence.write_bytes((json.dumps(result,ensure_ascii=False,indent=2)+'\n').encode());print(f'R6/R7 passed: {len(records)} CLI calls, {len(protocol)} MCP responses; {work}')
if __name__=='__main__':main()
