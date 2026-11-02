"""Independent pypdf/Poppler and Windows decoder QA for outputs of verify-r6-r7.py."""
from pathlib import Path
import argparse,base64,hashlib,json,queue,struct,subprocess,threading,time
from pypdf import PdfReader
from PIL import Image,ImageChops,ImageStat

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--verification',type=Path,required=True);ap.add_argument('--evidence',type=Path,required=True);args=ap.parse_args();run=json.loads(args.verification.read_text(encoding='utf-8'));work=Path(run['work']);out=args.evidence.parent/'pdf';out.mkdir(exist_ok=True);records=[]
    for layout,pages in [('slides',3),('notes',3),('handouts-3',1)]:
        path=work/f'pdf-{layout}.pdf';pdf=PdfReader(path);assert len(pdf.pages)==pages
        alltext='\n'.join(p.extract_text() or '' for p in pdf.pages);assert '真实' in alltext,alltext
        if layout=='notes':assert '中文备注' in alltext and 'PRIVATE SPEAKER NOTES' in alltext
        else:assert 'PRIVATE SPEAKER NOTES' not in alltext
        fonts=[]
        for page in pdf.pages:
            for f in page['/Resources'].get('/Font',{}).values():
                font=f.get_object();assert '/ToUnicode' in font
                for child in font.get('/DescendantFonts',[font]):
                    d=child.get_object().get('/FontDescriptor',{}).get_object();assert any(k in d for k in ['/FontFile','/FontFile2','/FontFile3']);fonts.append(str(font.get('/BaseFont')))
        assert len(pdf.outline)==3
        links=[str(a.get_object().get('/A',{}).get('/URI','')) for p in pdf.pages for a in p.get('/Annots',[])];assert 'https://example.com/joydeck' in links
        prefix=out/layout;subprocess.run(['pdftoppm','-r','72','-f','1','-singlefile','-png',str(path),str(prefix)],check=True,capture_output=True)
        records.append(dict(layout=layout,pages=pages,geometry=[float(v) for v in pdf.pages[0].mediabox],fonts=sorted(set(fonts)),embedded=True,to_unicode=True,bookmarks=len(pdf.outline),links=links,text=alltext))
    # Read the same page through Poppler; compare its artwork to the CPU reference at 72 dpi.
    cli=run['cli'];ref=work/'pdf-reference';subprocess.run([cli,'render',str(Path(__file__).resolve().parents[2]/'tests/fixtures/joydeck/r6-r7/delivery-media.pptx'),'--page','1','--output',str(ref)],check=True,capture_output=True)
    a=Image.open(out/'slides.png').convert('RGB');b=Image.open(ref/'slide-0001.png').convert('RGB');assert a.size==b.size
    diff=ImageChops.difference(a,b);mean=sum(ImageStat.Stat(diff).mean)/3;assert mean<5,mean
    # ISO BMFF metadata is read independently of the producer. The static MP4 has video and no audio.
    data=(work/'mp4-slides.mp4').read_bytes()
    def boxes(blob):
        pos=0
        while pos+8<=len(blob):
            size=struct.unpack_from('>I',blob,pos)[0];kind=blob[pos+4:pos+8].decode('ascii');head=8
            if size==1:size=struct.unpack_from('>Q',blob,pos+8)[0];head=16
            if size==0:size=len(blob)-pos
            assert size>=head and pos+size<=len(blob);yield kind,blob[pos+head:pos+size];pos+=size
    top=dict(boxes(data));assert 'ftyp' in top and 'moov' in top
    tracks=[]
    for kind,track in boxes(top['moov']):
        if kind!='trak':continue
        children=dict(boxes(track));mdia=dict(boxes(children['mdia']));handler=mdia['hdlr'][8:12].decode('ascii');tracks.append(handler)
    assert tracks==['vide'],tracks
    helper=Path(cli).parent/'joydeck-media-windows.exe'
    def decode(path):
        process=subprocess.Popen([str(helper),str(path)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,encoding='utf-8',creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0));q=queue.Queue()
        threading.Thread(target=lambda:[q.put(line) for line in process.stdout],daemon=True).start()
        def receive():
            reply=json.loads(q.get(timeout=15));assert 'error' not in reply,reply;return reply
        def command(value):process.stdin.write(json.dumps(value)+'\n');process.stdin.flush();return receive()
        try:
            info=receive();assert info['ready'] and info['video'];command(dict(command='play'));time.sleep(.35);command(dict(command='pause'));first=command(dict(command='frame'));assert first['width']<=640 and first['height']<=360
            pixels=base64.b64decode(first.pop('bgra'));assert len(pixels)==first['width']*first['height']*4 and len(set(pixels))>8
            command(dict(command='seek',seconds=.8));time.sleep(.15);second=command(dict(command='frame'));other=base64.b64decode(second.pop('bgra'));assert pixels!=other
            command(dict(command='volume',value=0));command(dict(command='play'));time.sleep(.15);playing=command(dict(command='status'));assert not playing['paused'];command(dict(command='pause'))
            return dict(info=info,frame=first,frame_sha256=hashlib.sha256(pixels).hexdigest(),seek_frame=second,seek_sha256=hashlib.sha256(other).hexdigest(),seek_changed=True)
        finally:process.stdin.close();process.wait(timeout=15);assert process.returncode==0
    media=decode(Path(__file__).resolve().parents[2]/'tests/fixtures/joydeck/r6-r7/original-h264-aac.mp4');video=decode(work/'mp4-slides.mp4')
    report=dict(cli_sha256=run['cli_sha256'],pdf=records,poppler_cpu_mean_absolute_channel_error=mean,mp4=dict(bytes=len(data),tracks=tracks,decoder=video),media=media)
    args.evidence.write_bytes((json.dumps(report,ensure_ascii=False,indent=2)+'\n').encode());print('Independent PDF/artwork/font/link and MP4/media decode passed',mean)
if __name__=='__main__':main()
