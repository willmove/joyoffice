"""R4/R5 executable, independent PPTX/workbook readback and resource preservation checks."""
from pathlib import Path
from io import BytesIO
import argparse,hashlib,json,subprocess,sys,tempfile,zipfile
from xml.etree import ElementTree as ET
ROOT=Path(__file__).resolve().parents[2]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--cli',type=Path,default=ROOT/'target/debug/joydeck-cli.exe');parser.add_argument('--evidence',type=Path,default=ROOT/'docs/joydeck/evidence/R4-R5/cli-verification.json');parser.add_argument('--iterations',type=int,default=40);args=parser.parse_args()
    sys.path.insert(0,str(ROOT/'target/corpus-tools'))
    from pptx import Presentation
    exe=args.cli.resolve();source=ROOT/'tests/fixtures/joydeck/r4-r5/independent-complex.pptx';work=Path(tempfile.mkdtemp(prefix='joydeck-r4-r5-'))/'中文 空格';work.mkdir();records=[]
    initial=hashlib.sha256(source.read_bytes()).hexdigest()
    def run(arguments,expected=0):
        r=subprocess.run([str(exe),*map(str,arguments)],capture_output=True,encoding='utf-8',timeout=120);records.append(dict(args=list(map(str,arguments)),exit_code=r.returncode,stdout=r.stdout,stderr=r.stderr));assert r.returncode==expected,(arguments,r.stderr);return r.stdout
    def edit(script,output,input_path=source,expected=0):
        path=work/f'script-{len(records)}.json';path.write_text(json.dumps(script,ensure_ascii=False),encoding='utf-8');return run(['edit',input_path,'--script',path,'--output',output],expected)
    def content(kind,**values):return dict(command='content',action=dict(kind=kind,**values))
    model=json.loads(run(['content',source]));info=json.loads(run(['inspect',source]));layouts=model['masters'][0]['layouts'];shape=info['slides'][0]['objects'][2]['id'];picture=next(s['id'] for s in info['slides'][1]['objects'] if s['kind']=='picture');chart=next(s['id'] for s in info['slides'][1]['objects'] if s['kind']=='chart')
    script=[content('theme-accent',slot=1,rgb=0x2578d0),content('theme-fonts',major='Arial',minor='Arial'),content('master-background',rgb=0xe8eff8),content('slide-size',width=1024.,height=576.),content('metadata',title='R5 edited corpus',author='JoyDeck tests'),dict(command='select',page=1,shapes=[shape]),content('shape-fill',rgb=0x22aa66),content('shape-preset',name='ellipse'),content('layout',id=layouts[0]['id']),dict(command='select',page=2,shapes=[picture]),content('picture',crop=[.12,.05,.08,.05],alpha=.75,brightness=.12,contrast=.05),dict(command='select',page=2,shapes=[chart]),content('chart-value',series=0,index=1,value=45.),content('chart-title',title='R5 live chart'),content('chart-type',chart_type='line'),dict(command='undo'),dict(command='redo'),dict(command='select',page=3,shapes=[]),content('background',rgb=0xfff1da),content('master-visibility',visible=False),dict(command='select',page=1,shapes=[shape]),content('shape-gradient',start_rgb=0x2266cc,end_rgb=0xaaddff,angle=45.)]
    outputs=[]
    for ext in ['pptx','joydeck']:
        output=work/f'edited.{ext}';edit(script,output);outputs.append(output);m=json.loads(run(['content',output]));assert m['slideSize']==dict(width=1024.,height=576.);assert m['props']['title']=='R5 edited corpus';assert m['props']['author']=='JoyDeck tests';assert m['masters'][0]['theme']['fonts']['minor']['latin']=='Arial';assert not m['slides'][2]['showMasterShapes'];c=next(s['kind'] for s in m['slides'][1]['shapes'] if s['id']==chart);assert c['type']=='chart' and c['title']=='R5 live chart' and c['series'][0]['values'][1]==45.,c
        run(['render',output,'--all','--output',work/f'render-{ext}'])
    independent=Presentation(outputs[0]);assert len(independent.slides)==3;assert abs(independent.slide_width/12700-1024.)<.001;assert independent.core_properties.title=='R5 edited corpus'
    selected=next(s for s in independent.slides[1].shapes if s.shape_id==picture);assert abs(selected.crop_left-.12)<1e-5
    selected=next(s for s in independent.slides[1].shapes if s.shape_id==chart).chart;assert selected.chart_title.text_frame.text=='R5 live chart';assert list(selected.series[0].values)[1]==45.
    external=selected.part._element.find('{http://schemas.openxmlformats.org/drawingml/2006/chart}externalData');assert external is not None
    rid=external.attrib['{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id'];workbook=selected.part.rels[rid].target_part.blob
    with zipfile.ZipFile(BytesIO(workbook)) as z:
        root=ET.fromstring(z.read('xl/worksheets/sheet1.xml'));ns={'s':'http://schemas.openxmlformats.org/spreadsheetml/2006/main'};assert root.find('.//s:c[@r="B3"]/s:v',ns).text=='45';assert root.find('.//s:c[@r="A3"]/s:is/s:t',ns).text=='Beta'
    touched={'[Content_Types].xml','ppt/presentation.xml','ppt/slideMasters/slideMaster1.xml','ppt/theme/theme1.xml','docProps/core.xml','ppt/slides/slide1.xml','ppt/slides/slide2.xml','ppt/slides/slide3.xml','ppt/slides/_rels/slide1.xml.rels','ppt/slides/_rels/slide2.xml.rels'}
    with zipfile.ZipFile(source) as a,zipfile.ZipFile(outputs[0]) as b:
        for name in a.namelist():
            if name not in touched:assert a.read(name)==b.read(name),name
    converted=work/'native-reexport.pptx';edit([],converted,outputs[1]);assert next(s.chart for s in Presentation(converted).slides[1].shapes if s.has_chart).series[0].values[1]==45.
    imported=work/'imported.pptx';run(['import-picture',source,ROOT/'tests/fixtures/joydeck/r4-r5/shapes.png','--output',imported]);assert len(Presentation(imported).slides[0].shapes)==len(independent.slides[0].shapes)+1
    for action in [content('picture',crop=[.8,0,.8,0],alpha=1,brightness=0,contrast=0),content('slide-size',width=-1,height=540),content('theme-accent',slot=7,rgb=0),content('chart-value',series=999,index=0,value=1)]:
        rejected=work/f'rejected-{len(records)}.pptx';edit([dict(command='select',page=2,shapes=[picture]),action],rejected,expected=1);assert not rejected.exists()
    replacement=work/'replacement.pptx'
    png=list((ROOT/'tests/fixtures/joydeck/r4-r5/shapes.png').read_bytes())
    edit([dict(command='select',page=2,shapes=[picture]),dict(command='insert-picture',png=png,x=0.,y=0.,width=240.,height=140.,replace=True)],replacement)
    assert next(s for s in Presentation(replacement).slides[1].shapes if s.shape_id==picture).image.blob==(ROOT/'tests/fixtures/joydeck/r4-r5/shapes.png').read_bytes()
    recovery_script=work/'recovery-script.json';recovery_script.write_text(json.dumps(script),encoding='utf-8');recovery_root=work/'recovery'
    probe=subprocess.Popen([str(exe),'checkpoint-probe',str(source),str(recovery_script),str(recovery_root)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8')
    try:
        import queue,threading
        ready=queue.Queue();threading.Thread(target=lambda:ready.put(probe.stdout.readline()),daemon=True).start();line=ready.get(timeout=45);assert line.startswith('READY '),line
    finally:probe.kill();probe.wait(timeout=15)
    entries=json.loads(run(['recovery-list',recovery_root]));entry=next(e for e in entries if not e['active'] and not e['problem']);recovered=work/'recovered.joydeck'
    run(['recover',recovery_root,entry['record']['session'],'--output',recovered]);back=json.loads(run(['content',recovered]));assert back['props']['title']=='R5 edited corpus';assert next(s['kind'] for s in back['slides'][1]['shapes'] if s['id']==chart)['series'][0]['values'][1]==45.
    unknown=work/'unknown-chart.pptx'
    with zipfile.ZipFile(source) as a,zipfile.ZipFile(unknown,'w',zipfile.ZIP_DEFLATED) as b:
        for name in a.namelist():
            data=a.read(name)
            if name=='ppt/charts/chart1.xml':data=data.replace(b'</c:chartSpace>',b'<c:extLst><c:ext uri="urn:future"/></c:extLst></c:chartSpace>')
            b.writestr(name,data)
    chart_script=[dict(command='select',page=2,shapes=[chart]),content('chart-value',series=0,index=1,value=66.)]
    rejected=work/'unknown-chart-must-not-exist.pptx';edit(chart_script,rejected,unknown,1);assert not rejected.exists();edit(chart_script,work/'unknown-chart-safe.joydeck',unknown)
    for page in range(1,4):run(['scene-bench',source,'--page',page,'--iterations',args.iterations,'--output',work/f'scene-{page}'])
    assert hashlib.sha256(source.read_bytes()).hexdigest()==initial
    report=dict(cli=str(exe),cli_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),source_sha256=initial,work=str(work),checks='public/native/reexport; independent python-pptx; embedded workbook cell B3; untouched original parts; image import; invalid commands atomic; fixed-threshold scene parity',records=records)
    args.evidence.parent.mkdir(parents=True,exist_ok=True);args.evidence.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8');print(f'R4/R5 verification passed: {len(records)} executable calls; {work}')
if __name__=='__main__':main()
