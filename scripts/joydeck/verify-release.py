"""Negative and rollback tests for offline release staging. Does not execute staged fixture binaries."""
import argparse,hashlib,importlib.util,json,shutil,tempfile,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('release',ROOT/'scripts/release-tool.py');release=importlib.util.module_from_spec(spec);spec.loader.exec_module(release)
def main():
 p=argparse.ArgumentParser();p.add_argument('--package',type=Path);p.add_argument('--output',type=Path,default=ROOT/'docs/evidence/m13/release-verification.json');a=p.parse_args();checks=[]
 with tempfile.TemporaryDirectory(prefix='joydeck-release-test-') as tmp:
  tmp=Path(tmp);releases=tmp/'versions'
  def fixture(name,reader=11):
   d=tmp/name;d.mkdir();(d/'joydeck').write_text('fixture only: '+name);(d/'BUILD-INFO.json').write_text(json.dumps({'nativeReaderMax':reader,'version':name}))
   manifest={f.name:release.digest(f) for f in d.iterdir()};(d/'SHA256.json').write_text(json.dumps(manifest))
   return Path(shutil.make_archive(str(d),'zip',root_dir=tmp,base_dir=name))
  def rejected(call,label):
   before=release.selection(releases)
   try:call()
   except (ValueError,OSError):pass
   else:raise AssertionError(label+' unexpectedly accepted')
   assert release.selection(releases)==before,label
   checks.append(label)
  one=fixture('joydeck-test-v1');two=fixture('joydeck-test-v2')
  rejected(lambda:release.install(one,releases,release.digest(one)),'unsigned_requires_explicit_flag')
  release.install(one,releases,release.digest(one),True);assert release.selection(releases)['current']=='joydeck-test-v1';checks.append('first_install')
  rejected(lambda:release.install(two,releases,'0'*64,True),'wrong_trusted_digest')
  release.install(two,releases,release.digest(two),True);assert release.selection(releases)['previous']=='joydeck-test-v1';checks.append('immutable_upgrade')
  release.install(two,releases,release.digest(two),True);checks.append('idempotent_same_release')
  release.rollback(releases);assert release.selection(releases)['current']=='joydeck-test-v1';checks.append('rollback_previous_intact')
  three=fixture('joydeck-test-v3');release.install(three,releases,release.digest(three),True,activate=False);assert release.selection(releases)['current']=='joydeck-test-v1';checks.append('stage_only_keeps_current')
  future=fixture('joydeck-test-old-reader',10);rejected(lambda:release.install(future,releases,release.digest(future),True),'incompatible_reader_rejected')
  for name,member,mode in [('traversal','joydeck-bad/../escape',0),('symlink','joydeck-bad/link',0o120777<<16),('absolute','/outside',0)]:
   bad=tmp/(name+'.zip')
   with zipfile.ZipFile(bad,'w') as z:
    info=zipfile.ZipInfo(member);info.external_attr=mode;z.writestr(info,b'outside')
   rejected(lambda:release.install(bad,releases,release.digest(bad),True),name+'_rejected')
  corrupt=fixture('joydeck-test-corrupt')
  with zipfile.ZipFile(corrupt,'a') as z:z.writestr('joydeck-test-corrupt/unindexed',b'new file')
  rejected(lambda:release.install(corrupt,releases,release.digest(corrupt),True),'unindexed_files_rejected')
  selected=release.selection(releases);previous=releases/selected['previous'];(previous/'joydeck').write_text('tampered')
  rejected(lambda:release.rollback(releases),'tampered_rollback_rejected')
  assert not list(releases.glob('.stage-*'));checks.append('staging_cleanup')
 result={'passed':True,'checks':checks,'executedStagedBinaries':False}
 if a.package:result['actualPackage']=release.verify(a.package)
 a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
if __name__=='__main__':main()
