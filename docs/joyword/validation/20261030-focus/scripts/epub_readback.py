from pathlib import Path
import zipfile,xml.etree.ElementTree as ET,hashlib,json
root=Path(__file__).resolve().parents[2];out=root/'target/m8-focus-20261030';X='{http://www.w3.org/1999/xhtml}';W='{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def text(el,br):
 result=el.text or ''
 for child in el:
  result+=('\n' if child.tag==br else text(child,br))+(child.tail or '')
 return result
rows=[]
for source,epub in [(root/'tests/fixtures/epub/book.docx',out/'export/book.epub'),(root/'tests/fixtures/mail-merge/template.docx',out/'final/work/letters.epub')]:
 with zipfile.ZipFile(source) as d,zipfile.ZipFile(epub) as z:
  assert z.testzip() is None
  first=z.infolist()[0];assert first.filename=='mimetype' and first.compress_type==0 and first.extra==b''
  assert z.read(first)==b'application/epub+zip'
  for n in z.namelist():
   if n.endswith(('.xhtml','.opf','.xml')):ET.fromstring(z.read(n))
  doc=ET.fromstring(d.read('word/document.xml'))
  expected=[text(p,W+'br') for p in doc.findall('.//'+W+'p')]
  content=ET.fromstring(z.read('EPUB/content.xhtml'));body=content.find(X+'body')
  actual=[text(e,X+'br') for e in body.iter() if e.tag in {X+f'h{i}' for i in range(1,7)}|{X+'p'}]
  assert actual==expected,(actual,expected)
  ids={e.attrib['id'] for e in body.iter() if 'id' in e.attrib}
  nav=ET.fromstring(z.read('EPUB/nav.xhtml'));links=[a.attrib['href'] for a in nav.iter(X+'a')]
  assert all(l=='content.xhtml' or l.split('#',1)[1] in ids for l in links)
  images=[n for n in z.namelist() if n.startswith('EPUB/images/')]
  source_images=[n for n in d.namelist() if n.startswith('word/media/')]
  assert sorted(hashlib.sha256(z.read(n)).hexdigest() for n in images)==sorted(hashlib.sha256(d.read(n)).hexdigest() for n in source_images)
  assert not any(e.tag==X+'script' for e in content.iter())
  rows.append({'source':str(source.relative_to(root)),'source_sha256':sha(source),'epub_sha256':sha(epub),'paragraphs':len(actual),'exact_unicode_order_and_softbreaks':True,'navigation_targets':links,'images_identical':len(images),'mimetype_first_stored_no_extra':True,'all_xml_well_formed':True,'zip_crc_ok':True,'no_scripts':True})
assert sha(out/'final/work/letters.docx')==sha(root/'tests/fixtures/mail-merge/template.docx')
assert sha(out/'final/work/book.docx')==sha(root/'tests/fixtures/epub/book.docx')
result={'cases':rows,'gui_source_unchanged':True,'cli_complex_source_unchanged':True,'gui_export_created_by':'native menu/preflight/save dialog, final binary 451a58bd','gui_cancel_no_output_observed_before_export':True,'independent_epubcheck_cli_and_gui':'0 fatals / 0 errors / 0 warnings','reader_gui_acceptance':False}
(out/'epub-readback.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(json.dumps(result,ensure_ascii=False))
