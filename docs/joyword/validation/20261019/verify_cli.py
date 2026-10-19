from pathlib import Path
import subprocess,json,hashlib,platform,sys
root=Path.cwd(); out=root/'target/validation-20261019/cli-final'; out.mkdir(parents=True,exist_ok=True)
cli=root/'target/debug/joyword-cli.exe'
report={'source':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'platform':platform.platform(),'font_sha256':hashlib.sha256(Path('C:/Windows/Fonts/simsun.ttc').read_bytes()).hexdigest(),'cases':[]}
def run(args):
 p=subprocess.run([str(cli),*map(str,args)],capture_output=True,timeout=120)
 return p.returncode,p.stdout,p.stderr
for source in sorted((root/'tests/fixtures/docx').glob('DOCX-MIN-*.docx')):
 before=hashlib.sha256(source.read_bytes()).hexdigest(); case={'id':source.stem,'input_sha256':before,'runs':[]}
 for n in [1,2]:
  dest=out/source.stem/str(n); dest.mkdir(parents=True,exist_ok=True)
  for op,args in [('inspect',['inspect',source]),('render',['render',source,'--font','C:/Windows/Fonts/simsun.ttc:0','--output',dest/'png','--dpi','96']),('pdf',['convert',source,'--to','pdf','--font','C:/Windows/Fonts/simsun.ttc:0','--output',dest/'document.pdf'])]:
   rc,stdout,stderr=run(args); (dest/(op+'.json')).write_bytes(stdout); (dest/(op+'.stderr')).write_bytes(stderr)
   case['runs'].append({'run':n,'op':op,'exit':rc}); assert rc==0,(source,op,rc,stderr)
 a=out/source.stem/'1'; b=out/source.stem/'2'
 hashes={str(p.relative_to(a)):hashlib.sha256(p.read_bytes()).hexdigest() for p in a.rglob('*') if p.is_file()}
 case['repeatable']=all((b/p).read_bytes()==(a/p).read_bytes() for p in hashes)
 case['artifacts']=hashes; case['input_unchanged']=before==hashlib.sha256(source.read_bytes()).hexdigest()
 assert case['repeatable'] and case['input_unchanged']
 report['cases'].append(case)
rc,stdout,stderr=run(['inspect',root/'tests/fixtures/docx/DOCX-MIN-001.docx','--max-text-bytes','1']); truncated=json.loads(stdout)
report['truncated_exit']=rc;report['truncated_report']=truncated
assert rc==3 and truncated['truncated']
(out/'summary.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'cases':len(report['cases']),'all_repeatable':True,'truncation_exit':rc}))
