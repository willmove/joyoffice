from pathlib import Path
import subprocess,json,sys
import pdfplumber
from pypdf import PdfReader
from PIL import Image,ImageChops
out=Path('target/validation-20261019/cli-final'); reports=[]
for case in sorted(out.glob('DOCX-MIN-*')):
 run=case/'1'; pdf=run/'document.pdf'
 poppler=Path('C:/Users/willm/.cache/codex-runtimes/codex-primary-runtime/dependencies/native/poppler/Library/bin')
 subprocess.run([str(poppler/'pdftoppm.exe'),'-r','96','-png',str(pdf),str(run/'pdf')],check=True,capture_output=True)
 with pdfplumber.open(pdf) as doc: a='\n'.join(p.extract_text() or '' for p in doc.pages)
 c='\n'.join(p.extract_text() or '' for p in PdfReader(pdf).pages)
 norm=lambda s:''.join(s.split())
 assert norm(a)==norm(c),(case.name,a,c)
 (run/'pdfplumber.txt').write_text(a,encoding='utf-8')
 pages=[]
 for png in sorted((run/'png').glob('page-*.png')):
  n=int(png.stem.split('-')[-1]); rendered=list(run.glob(f'pdf-{n}.png')) or list(run.glob(f'pdf-{n:02}.png'))
  cpu=Image.open(png).convert('RGB'); ref=Image.open(rendered[0]).convert('RGB')
  # Consumers can round the page edge by one pixel; compare content ink bounding boxes.
  bbox=lambda im:ImageChops.difference(im,Image.new('RGB',im.size,'white')).getbbox()
  pages.append({'page':n,'cpu_size':cpu.size,'pdf_size':ref.size,'cpu_ink_box':bbox(cpu),'pdf_ink_box':bbox(ref)})
 reports.append({'id':case.name,'independent_text_readers_agree':True,'pages':pages})
(out/'pdf-readback.json').write_text(json.dumps(reports,indent=2),encoding='utf-8');print(json.dumps(reports))
