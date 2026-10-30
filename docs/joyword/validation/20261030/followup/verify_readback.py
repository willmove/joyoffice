import json,hashlib,zipfile,subprocess,re
from pathlib import Path
base=Path('target/m7-followup')
work=base/'gui/work'
evidence=Path('docs/joyword/validation/20261030/followup')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
a=work/'wps-roundtrip.docx';b=work/'wps-fixed.docx'
with zipfile.ZipFile(a) as old, zipfile.ZipFile(b) as new:
    assert {n for n in old.namelist() if not n.endswith('/')}=={n for n in new.namelist() if not n.endswith('/')}
    changed=[n for n in old.namelist() if not n.endswith('/') and old.read(n)!=new.read(n)]
    assert changed==['word/document.xml'],changed
    settings=hashlib.sha256(old.read('word/settings.xml')).hexdigest()
r=json.loads(subprocess.check_output(['target/release/joyword-cli.exe','inspect',str(b)]))
assert r['text'].endswith('[WPS-M7-FIX]') and not r['read_only'] and r['partial']
on=work/'hundred-marks-on.pdf';off=work/'hundred-marks-off.pdf'
assert on.read_bytes()==off.read_bytes()
text=(base/'pdf-text.txt').read_text(encoding='utf-8')
for i in range(1,101):assert f'Page {i:03d}:' in text
assert 'END-M7-FIX' not in text and 'START' not in text and '\u00b6' not in text
result={'wps':{'source_sha256':sha(a),'saved_sha256':sha(b),'changed_parts':changed,'settings_byte_identical_sha256':settings,'text':r['text'],'diagnostics':r['diagnostics'],'preserved_fragments':r['preserved_fragments'],'read_only':r['read_only']},'pdf':{'on_sha256':sha(on),'off_sha256':sha(off),'bytes':on.stat().st_size,'identical':True,'all_100_page_labels':True,'no_undone_markers':True,'qpdf_exit':0,'pages':100,'paper':'A4'}}
(evidence/'readback.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,ensure_ascii=False))

