import hashlib,json,subprocess
from pathlib import Path
root=Path.cwd();out=root/'target/m9-20261030';cli=root/'target/debug/joyword-cli.exe'
java=Path('C:/Applications/Java/jdk-24.0.1/bin/java.exe');validator=root/'target/m8-archive-20261030/verapdf-validated.jar'
assert hashlib.sha256(validator.read_bytes()).hexdigest()=='287cc9c9757f99dcff4a2d39b68268245b48103fb6af443162d5a29df2afa332'
cases=['tests/fixtures/mail-merge/template.docx']+[f'tests/fixtures/feature/DOCX-FEAT-{s}.docx' for s in ['T200-NUM','T201-TABLE','T202-PAGES','T203-IMAGES','T300-REVIEW']]
results=[]
for source in cases:
 p=root/source;dest=out/(p.stem+'.pdf');cmd=[str(cli),'convert',str(p),'--to','pdfa-2b','--archive-year','2026','--font','C:/Windows/Fonts/simsun.ttc:0','--output',str(dest)]
 before=hashlib.sha256(p.read_bytes()).hexdigest();r=subprocess.run(cmd,capture_output=True,text=True,encoding='utf-8',errors='replace');row=dict(source=source,source_sha256=before,command=cmd,returncode=r.returncode,stdout=r.stdout,stderr=r.stderr,source_unchanged=hashlib.sha256(p.read_bytes()).hexdigest()==before)
 if r.returncode==0:
  check=[str(java),'-jar',str(validator),'--flavour','2b','--format','json',str(dest)];v=subprocess.run(check,capture_output=True,text=True,encoding='utf-8',errors='replace');(out/(p.stem+'-verapdf.json')).write_text(v.stdout,encoding='utf-8');row['validator_command']=check;row['validator_exit']=v.returncode;row['pdf_sha256']=hashlib.sha256(dest.read_bytes()).hexdigest()
  try:
   job=json.loads(v.stdout)['report']['jobs'][0];row['validation']=job['validationResult']
  except Exception as e: row['validator_error']=str(e)
 else: row['no_output']=not dest.exists()
 results.append(row)
 print(p.name,r.returncode,row.get('validation',r.stderr.strip()),flush=True)
(out/'matrix.json').write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf-8')
