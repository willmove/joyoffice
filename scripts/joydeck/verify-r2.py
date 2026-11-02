"""Exercise the actual R2 executable, public OOXML, native v2 and forced-process recovery."""
from pathlib import Path
import argparse, hashlib, json, queue, shutil, struct, subprocess, tempfile, threading, zipfile
from xml.etree import ElementTree as ET

ROOT=Path(__file__).resolve().parents[2]
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli',type=Path,default=ROOT/'target/debug/joydeck-cli.exe')
    parser.add_argument('--evidence',type=Path,default=ROOT/'docs/joydeck/evidence/R2/cli-verification.json')
    args=parser.parse_args();exe=args.cli.resolve()
    work=Path(tempfile.mkdtemp(prefix='joydeck-r2-'))/'中文 空格路径';work.mkdir()
    source=work/'真实 原稿.pptx'
    with zipfile.ZipFile(ROOT/'tests/fixtures/joydeck/independent.pptx') as z,zipfile.ZipFile(source,'w',zipfile.ZIP_DEFLATED) as out:
        for item in z.infolist():out.writestr(item.filename,z.read(item))
        out.writestr('vendor/opaque.xml',b'<?xml version="1.0"?><vendor:keep xmlns:vendor="urn:joydeck-test" future="true"><![CDATA[keep exactly]]></vendor:keep>')
    initial=hashlib.sha256(source.read_bytes()).hexdigest();records=[]
    def run(arguments,expected=0):
        r=subprocess.run([str(exe),*map(str,arguments)],cwd=work,capture_output=True,text=True,encoding='utf-8',timeout=40)
        assert r.returncode==expected,(arguments,r.returncode,r.stderr)
        records.append(dict(args=list(map(str,arguments)),exit_code=r.returncode,stdout=r.stdout,stderr=r.stderr));return r.stdout
    inspect=json.loads(run(['inspect',source]));assert len(inspect['slides'])==3
    title=inspect['slides'][0]['objects'][0]['id'];old_transform=inspect['slides'][0]['objects'][0]['transform']
    assert 'undo' in json.loads(run(['commands']))
    text='JoyDeck R2 edited / 已编辑 😀'
    commands=[{'command':'select','page':1,'shapes':[title]},{'command':'replace-text','text':text},
              {'command':'transform','dx':12,'dy':6,'rotation':15},{'command':'insert-rect','x':400,'y':350,'width':120,'height':60},
              {'command':'undo'},{'command':'redo'}]
    script=work/'修改.json';script.write_text(json.dumps(commands,ensure_ascii=False),encoding='utf-8')
    outputs=[]
    for ext in ['pptx','joydeck']:
        output=work/('已编辑.'+ext);run(['edit',source,'--script',script,'--output',output]);outputs.append(output)
        info=json.loads(run(['inspect',output]));assert info['slides'][0]['objects'][0]['text']==text
        assert len(info['slides'][0]['objects'])==5
        t=info['slides'][0]['objects'][0]['transform'];assert abs(t['x']-old_transform['x']-12)<.0001 and t['rot']==15
        digest=hashlib.sha256(output.read_bytes()).hexdigest()
        run(['edit',source,'--script',script,'--output',output],1);assert hashlib.sha256(output.read_bytes()).hexdigest()==digest
    pptx=outputs[0]
    with zipfile.ZipFile(source) as before,zipfile.ZipFile(pptx) as after:
        xml=ET.fromstring(after.read('ppt/slides/slide1.xml'))
        assert text in [e.text for e in xml.iter('{http://schemas.openxmlformats.org/drawingml/2006/main}t')]
        for name in before.namelist():
            if name!='ppt/slides/slide1.xml':assert before.read(name)==after.read(name),name
        assert 'presentation.json' not in after.namelist()
    run(['render',pptx,'--output',work/'rendered','--all'])
    images=sorted((work/'rendered').glob('*.png'));assert len(images)==3
    for path in images:assert struct.unpack('>II',path.read_bytes()[16:24])==(960,540)
    empty=work/'empty.json';empty.write_text('[]',encoding='utf-8')
    run(['edit',outputs[1],'--script',empty,'--output',work/'native-to-pptx.pptx'])
    again=json.loads(run(['inspect',work/'native-to-pptx.pptx']));assert again['slides'][0]['objects'][0]['text']==text
    new=work/'new.json';new.write_text(json.dumps([{'command':'insert-rect','x':10,'y':10,'width':60,'height':40},{'command':'new-slide'},{'command':'insert-text','x':40,'y':40,'text':'New document'}]),encoding='utf-8')
    run(['edit','--new','--script',new,'--output',work/'new.joydeck'])
    assert len(json.loads(run(['inspect',work/'new.joydeck']))['slides'])==2
    bad=work/'invalid.json';bad.write_text(json.dumps([{'command':'insert-rect','x':0,'y':0,'width':-1,'height':2}]),encoding='utf-8')
    run(['edit',source,'--script',bad,'--output',work/'must-not-exist.pptx'],1);assert not (work/'must-not-exist.pptx').exists()
    recovery=work/'recovery';ready=queue.Queue()
    child=subprocess.Popen([str(exe),'checkpoint-probe',str(source),str(script),str(recovery)],cwd=work,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8')
    threading.Thread(target=lambda:ready.put(child.stdout.readline()),daemon=True).start()
    try:
        line=ready.get(timeout=20);assert line.startswith('READY '),line
        session=line.strip().split(' ',1)[1]
        assert json.loads(run(['recovery-list',recovery]))[0]['active']
    finally:
        child.kill();child.wait(timeout=5)
    rows=json.loads(run(['recovery-list',recovery]));record=next(r for r in rows if r['record']['session']==session);assert not record['active'] and record['problem'] is None
    recovered=work/'崩溃 恢复.joydeck';run(['recover',recovery,session,'--output',recovered])
    restored=json.loads(run(['inspect',recovered]));assert restored['slides'][0]['objects'][0]['text']==text
    # A bad/future snapshot and an oversized index must not block the valid recovery.
    with zipfile.ZipFile(recovered) as z,zipfile.ZipFile(recovery/'9-9-9-1.joydeck','w') as out:
        for name in z.namelist():
            data=z.read(name)
            if name=='manifest.json':m=json.loads(data);m['version']=999;data=json.dumps(m).encode()
            out.writestr(name,data)
    (recovery/'9-9-9-1.json').write_text(json.dumps(dict(session='9-9-9',revision=1,source=None,source_fingerprint=None,created=1,snapshot='9-9-9-1.joydeck')),encoding='utf-8')
    (recovery/'bad.json').write_bytes(b'x'*70000)
    rows=json.loads(run(['recovery-list',recovery]));assert any(r['problem'] for r in rows) and any(r['record']['session']==session and r['problem'] is None for r in rows)
    assert (recovery/'9-9-9-1.bad').exists() and (recovery/'bad.index-bad').exists()
    assert hashlib.sha256(source.read_bytes()).hexdigest()==initial
    diagnostics=json.loads(run(['diagnostics']));assert diagnostics['stage']=='R2' and not diagnostics['cli_uses_gui']
    report=dict(cli=str(exe),cli_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),work=str(work),checks=len(records),passed=True,forced_crash_pid=child.pid,recovered_session=session,
                source_sha256=initial,outputs={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in outputs+images},runs=records)
    args.evidence.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(f'R2 verification passed: {len(records)} commands; forced crash and recovery; {work}')
if __name__=='__main__':main()
