from pathlib import Path
import json,hashlib,zipfile,subprocess,sys
root=Path(__file__).resolve().parents[2];out=root/'target/m8-focus-20261030';artifact=out/'JoyWord-preview-0.0.1-m8-epub-windows-x64.zip'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(artifact)==sha(out/'JoyWord-preview-repeat.zip')
dest=out/'unpacked';dest.mkdir(exist_ok=True)
with zipfile.ZipFile(artifact) as z:
 assert z.testzip() is None
 for n in z.namelist():
  assert n.startswith('JoyWord-preview/') and '..' not in Path(n).parts and not Path(n).is_absolute()
 manifest=json.loads(z.read('JoyWord-preview/manifest.json'))
 for n,digest in manifest['files'].items():assert hashlib.sha256(z.read('JoyWord-preview/'+n)).hexdigest()==digest,n
 inventory=json.loads(z.read('JoyWord-preview/source-inventory.spdx.json'))
 sys.path.insert(0,str(root/'target/m7-20261030/schema-validator'));import jsonschema
 schema=json.loads((root/'target/m7-20261030/spdx-2.3.schema.json').read_text(encoding='utf-8'))
 jsonschema.Draft7Validator(schema).validate(inventory)
 assert not manifest['stable_release_approved']
 z.extractall(dest);members=len(z.namelist())
 binary=dest/'JoyWord-preview/joyword-cli.exe'
 assert sha(binary)==sha(root/'target/release/joyword-cli.exe')
 cmd=[str(binary),'epub',str(root/'tests/fixtures/epub/book.docx'),'--plan','--language','zh-CN','--modified','2026-10-30T10:00:00Z']
 plan=subprocess.run(cmd,capture_output=True,text=True,encoding='utf-8',check=True)
 data=json.loads(plan.stdout);assert data['writes']==0
 (out/'unpacked-epub-plan.json').write_text(plan.stdout,encoding='utf-8')
 result={'archive_sha256':sha(artifact),'bytes':artifact.stat().st_size,'repeat_byte_identical':True,'members':members,'all_member_hashes_verified':True,'spdx_schema_validated':True,'spdx_packages':len(inventory['packages']),'spdx_relationships':len(inventory['relationships']),'missing_notices':len(manifest['missing_dependency_notices']),'stable_release_approved':False,'source':manifest['source'],'unpacked_cli_plan_writes':data['writes'],'unpacked_cli_sha256':sha(binary),'gui_sha256':sha(dest/'JoyWord-preview/joyword.exe')}
 (out/'package-readback.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(json.dumps(result))
