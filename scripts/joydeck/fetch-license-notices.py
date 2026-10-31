"""Fetch missing license notices from the exact Cargo archive VCS commit, never from a moving branch."""
import argparse,concurrent.futures,hashlib,json,re,urllib.request,urllib.error
from pathlib import Path
from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
def main():
 p=argparse.ArgumentParser();p.add_argument('metadata',type=Path);p.add_argument('index',type=Path);a=p.parse_args();metadata=json.loads(a.metadata.read_text(encoding='utf-8'));packages={(p['name'],p['version']):p for p in metadata['packages']}
 missing=[v for v in json.loads(a.index.read_text(encoding='utf-8')) if not v['texts'] and not v['workspace']]
 groups={};unresolved=[]
 for row in missing:
  package=packages[row['name'],row['version']];source=Path(package['manifest_path']).parent;vcs=source/'.cargo_vcs_info.json';v=json.loads(vcs.read_text(encoding='utf-8')) if vcs.exists() else {};commit=v.get('git',{}).get('sha1')
  repo=package.get('repository') or package.get('homepage') or ''
  match=re.search(r'github.com/([^/]+)/([^/#?]+)',repo)
  if not match or not commit:unresolved.append({'package':row['name']+'-'+row['version'],'reason':'no pinned GitHub source metadata'});continue
  repository=match.group(1)+'/'+match.group(2).removesuffix('.git')
  groups.setdefault((repository,commit),[]).append(row)
 patterns=['LICENSE-MIT','LICENSE-APACHE','LICENSE','LICENSE.md','LICENCE','UNLICENSE','COPYING','NOTICE']
 def fetch(key):
  repository,commit=key;files=[]
  for name in patterns:
   url=f'https://raw.githubusercontent.com/{repository}/{commit}/{name}'
   try:
    with urllib.request.urlopen(url,timeout=12) as response:data=response.read(1024*1024+1)
   except (urllib.error.URLError,TimeoutError):continue
   if not data or len(data)>1024*1024:continue
   files.append((name,data,url))
  return key,files
 downloaded=0
 with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
  for key,files in pool.map(fetch,groups):
   for row in groups[key]:
    if not files:unresolved.append({'package':row['name']+'-'+row['version'],'reason':'no root notice at pinned commit'});continue
    folder=LICENSES / 'dependency-notices'/(row['name']+'-'+row['version']);folder.mkdir(parents=True,exist_ok=True);sources=[]
    for name,data,url in files:
     (folder/name).write_bytes(data);sources.append({'file':name,'url':url,'sha256':hashlib.sha256(data).hexdigest()})
    (folder/'SOURCES.json').write_text(json.dumps(sources,indent=2)+'\n', encoding='utf-8');downloaded+=1
 result={'packagesFilled':downloaded,'unresolved':unresolved,'policy':'exact VCS commit from Cargo package metadata; no moving-branch notices'}
 (EVIDENCE / 'm13').mkdir(parents=True,exist_ok=True)
 (EVIDENCE / 'm13/license-notices.json').write_text(json.dumps(result,indent=2)+'\n', encoding='utf-8');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
