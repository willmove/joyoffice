import argparse,pathlib,zipfile,xml.etree.ElementTree as E,subprocess,os,json,hashlib
parser=argparse.ArgumentParser(description="Compare actual Word style responses to a controlled theme change")
parser.add_argument("--output",type=pathlib.Path,required=True,help="New output directory; existing directories are refused")
parser.add_argument("--binary",type=pathlib.Path,required=True,help="Compiled style-audit example")
args=parser.parse_args()
repo=pathlib.Path(__file__).resolve().parents[2]
root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
binary=args.binary.resolve()
(root/'saved').mkdir()
a='{http://schemas.openxmlformats.org/drawingml/2006/main}'
paths=sorted((repo/'tests/fixtures/reference/word2024/breaks').glob('*.docx'))+[repo/'tests/fixtures/reference/word2024/interop/word_edited.docx']+[repo/f'tests/fixtures/reference/word2024/interop/wordresaved/DOCX-MIN-00{i}.docx' for i in range(1,5)]+[repo/'tests/fixtures/reference/word2024/interop/wordresaved/DOCX-SAMPLE-001.docx']
with (root/'effective-styles.log').open('w') as log:
 subprocess.run([str(binary),*map(str,paths)],env={**os.environ,'JOYWORD_STYLE_AUDIT_OUT':str(root/'saved')},stdout=log,stderr=subprocess.STDOUT,check=True)
def changed_theme(source,target):
 with zipfile.ZipFile(source) as zi,zipfile.ZipFile(target,'w') as zo:
  for name in zi.namelist():
   data=zi.read(name)
   if name.startswith('word/theme/') and name.endswith('.xml'):
    theme=E.fromstring(data)
    theme.find('.//'+a+'majorFont/'+a+'latin').set('typeface','Audit Heading')
    theme.find('.//'+a+'minorFont/'+a+'latin').set('typeface','Audit Body')
    accent=theme.find('.//'+a+'accent1/'+a+'srgbClr');accent.set('val','FF0000')
    data=E.tostring(theme,encoding='utf-8',xml_declaration=True)
   zo.writestr(name,data)
def values(path):
 r=subprocess.run([str(binary),str(path)],capture_output=True,text=True,env={k:v for k,v in {**os.environ,'JOYWORD_STYLE_AUDIT_VALUES':'1'}.items() if k!='JOYWORD_STYLE_AUDIT_OUT'},check=True)
 (path.with_suffix('.log')).write_text(r.stdout)
 return dict(line[6:].split(' font=',1) for line in r.stdout.splitlines() if line.startswith('VALUE '))
rows=[]
for i,src in enumerate(paths):
 original_hash=hashlib.sha256(src.read_bytes()).hexdigest()
 x=root/f'{i:02}-source-theme-changed.docx';y=root/f'{i:02}-saved-theme-changed.docx'
 changed_theme(src,x);changed_theme(root/'saved'/f'{i:02}.docx',y)
 before=values(x);after=values(y)
 different=[{'style':key,'from_original':before[key],'from_saved':after.get(key)} for key in before if before[key]!=after.get(key)]
 rows.append({'path':str(src.relative_to(repo)),'source_sha256':original_hash,'source_unchanged':hashlib.sha256(src.read_bytes()).hexdigest()==original_hash,'styles':len(before),'changed_theme_differences':different})
 print(src.name,len(before),len(different))
(root/'theme-response.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2))
