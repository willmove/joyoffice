"""Produce an SPDX 2.3 dependency inventory from the locked, target-filtered Cargo graph."""
import datetime,hashlib,json,subprocess,tomllib,shutil,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
def generate(output,target,profile='release'):
 data=json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--filter-platform',target,'--format-version','1'],cwd=ROOT))
 nodes={v['id']:v for v in data['resolve']['nodes']};packages={v['id']:v for v in data['packages']};work={data['resolve']['root']} if data['resolve']['root'] else {v['id'] for v in data['packages'] if v['name']=='joydeck'}
 reached=set()
 while work:
  identifier=work.pop()
  if identifier in reached:continue
  reached.add(identifier);work.update(d['pkg'] for d in nodes[identifier]['deps'] if any(k['kind']!='dev' for k in d['dep_kinds']))
 ids={v:'SPDXRef-'+hashlib.sha256(v.encode()).hexdigest()[:20] for v in reached}
 lock=hashlib.sha256((ROOT/'Cargo.lock').read_bytes()).hexdigest()
 now=datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
 objects=[]
 license_root=Path(output).parent/'dependency-licenses';license_root.mkdir()
 license_index=[]
 lockdata=tomllib.loads((ROOT/'Cargo.lock').read_text())
 checksums={(p['name'],p['version'],p.get('source')):p.get('checksum') for p in lockdata['package']}
 for identifier in sorted(reached):
  p=packages[identifier];checksum=checksums.get((p['name'],p['version'],p.get('source')))
  row={'SPDXID':ids[identifier],'name':p['name'],'versionInfo':p['version'],'downloadLocation':('https://crates.io/api/v1/crates/'+p['name']+'/'+p['version']+'/download') if p.get('source','') and p['source'].startswith('registry+') else 'NOASSERTION','filesAnalyzed':False,'licenseDeclared':(p.get('license') or 'NOASSERTION').replace('/',' OR '),'licenseConcluded':'NOASSERTION','copyrightText':'NOASSERTION'}
  if checksum:row['checksums']=[{'algorithm':'SHA256','checksumValue':checksum}]
  source=Path(p['manifest_path']).parent.resolve();candidates=[v for v in source.iterdir() if v.is_file() and not v.is_symlink() and v.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','UNLICENSE'))]
  if p.get('license_file'):
   candidate=(source/p['license_file']).resolve()
   if candidate.is_relative_to(source) and candidate.is_file():candidates.append(candidate)
  copied=[]
  for candidate in sorted(set(candidates)):
   if candidate.stat().st_size>2*1024*1024:continue
   folder=license_root/(p['name']+'-'+p['version']);folder.mkdir(exist_ok=True)
   destination=folder/candidate.relative_to(source);destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(candidate,destination);copied.append(destination.relative_to(Path(output).parent).as_posix())
  supplement=ROOT/'licenses/dependency-notices'/(p['name']+'-'+p['version'])
  if supplement.is_dir():
   folder=license_root/(p['name']+'-'+p['version']);folder.mkdir(exist_ok=True)
   for candidate in supplement.iterdir():
    if candidate.is_file():destination=folder/candidate.name;shutil.copy2(candidate,destination);copied.append(destination.relative_to(Path(output).parent).as_posix())
  license_index.append({'name':p['name'],'version':p['version'],'declared':p.get('license'),'texts':copied,'workspace':p.get('source') is None})
  objects.append(row)
 relationships=[{'spdxElementId':'SPDXRef-DOCUMENT','relationshipType':'DESCRIBES','relatedSpdxElement':ids[v]} for v in reached if packages[v]['name']=='joydeck']
 for identifier in sorted(reached):
  for dep in nodes[identifier]['deps']:
   if dep['pkg'] in reached and any(k['kind']!='dev' for k in dep['dep_kinds']):relationships.append({'spdxElementId':ids[identifier],'relationshipType':'DEPENDS_ON','relatedSpdxElement':ids[dep['pkg']]})
 sbom={'spdxVersion':'SPDX-2.3','dataLicense':'CC0-1.0','SPDXID':'SPDXRef-DOCUMENT','name':'JoyDeck development dependency inventory '+target,'documentNamespace':'urn:uuid:'+str(uuid.uuid5(uuid.NAMESPACE_URL,'joydeck/'+target+'/'+lock+'/'+now)),'creationInfo':{'created':now,'creators':['Tool: joydeck-sbom-1']},'packages':objects,'relationships':relationships,'comment':'Locked target-filtered Cargo normal/build graph with default desktop feature. Declared licenses are metadata, not conclusions. System frameworks/fonts and QA tools are listed separately in THIRD_PARTY.md.'}
 Path(output).write_text(json.dumps(sbom,indent=2)+'\n')
 (license_root/'index.json').write_text(json.dumps(license_index,indent=2)+'\n')
 return {'packages':len(objects),'lockSha256':lock,'target':target,'profile':profile}
if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--target',required=True);a=p.parse_args();print(json.dumps(generate(a.output,a.target),indent=2))
