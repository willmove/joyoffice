"""Verify and stage immutable offline development releases, then atomically select or roll back.
A SHA256 must come from a separately trusted source. Official macOS verification additionally requires
a pinned Team ID. This tool never downloads or launches applications.
"""
import argparse,contextlib,hashlib,json,os,re,shutil,stat,subprocess,sys,tempfile,zipfile
from pathlib import Path,PurePosixPath
MAX_FILES=4096;MAX_BYTES=512*1024*1024
def digest(path):
 h=hashlib.sha256()
 with Path(path).open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 return h.hexdigest()
def safe_name(name):
 path=PurePosixPath(name)
 if not name or path.is_absolute() or any(p in ('','.','..') or ':' in p or '\\' in p for p in name.rstrip('/').split('/')):raise ValueError('Unsafe release path')
 return path
def verify(directory):
 root=Path(directory)
 manifest=json.loads((root/'SHA256.json').read_text(encoding='utf-8'))
 if not isinstance(manifest,dict) or not 1<=len(manifest)<MAX_FILES:raise ValueError('Invalid file manifest')
 entries=list(root.rglob('*'))
 if any(p.is_symlink() for p in entries):raise ValueError('Release contains a symbolic link')
 actual={p.relative_to(root).as_posix() for p in entries if p.is_file()}
 if actual!=set(manifest)|{'SHA256.json'}:raise ValueError('Manifest file set mismatch')
 for name,expected in manifest.items():
  safe_name(name);path=root/name
  if path.is_symlink() or not re.fullmatch('[0-9a-f]{64}',expected) or digest(path)!=expected:raise ValueError('Release hash mismatch: '+name)
 info=json.loads((root/'BUILD-INFO.json').read_text(encoding='utf-8'))
 if info['nativeReaderMax']<11:raise ValueError('Release cannot read current documents')
 return {'filesVerified':len(manifest),'build':info}
def extract(archive,dest):
 with zipfile.ZipFile(archive) as z:
  infos=z.infolist()
  if len(infos)>MAX_FILES or sum(i.file_size for i in infos)>MAX_BYTES:raise ValueError('ZIP budget exceeded')
  names=set();tops=set()
  for i in infos:
   path=safe_name(i.filename)
   if i.orig_filename!=i.filename or i.filename in names or stat.S_ISLNK(i.external_attr>>16):raise ValueError('Duplicate or symbolic ZIP entry')
   if i.file_size>128*1024*1024 or i.file_size>max(1,i.compress_size)*1000:raise ValueError('ZIP part budget exceeded')
   names.add(i.filename);tops.add(path.parts[0])
  if len(tops)!=1:raise ValueError('Expected one package root')
  name=next(iter(tops))
  if not re.fullmatch('joydeck-[a-z0-9-]+',name):raise ValueError('Invalid release directory name')
  for i in infos:
   target=Path(dest).joinpath(*PurePosixPath(i.filename).parts)
   if i.is_dir():target.mkdir(parents=True,exist_ok=True);continue
   target.parent.mkdir(parents=True,exist_ok=True)
   with z.open(i) as src,target.open('xb') as out:shutil.copyfileobj(src,out,1024*1024);out.flush();os.fsync(out.fileno())
   target.chmod(0o755 if (i.external_attr>>16)&0o111 else 0o644)
 return Path(dest)/name
@contextlib.contextmanager
def locked(root):
 root=Path(root);root.mkdir(parents=True,exist_ok=True)
 with (root/'release.lock').open('a+b') as file:
  if os.name=='posix':
   import fcntl;fcntl.flock(file,fcntl.LOCK_EX)
  else:
   import msvcrt
   file.seek(0);file.write(b'0');file.flush();file.seek(0);msvcrt.locking(file.fileno(),msvcrt.LK_LOCK,1)
  try:yield
  finally:
   if os.name=='posix':fcntl.flock(file,fcntl.LOCK_UN)
   else:file.seek(0);msvcrt.locking(file.fileno(),msvcrt.LK_UNLCK,1)
def atomic_json(path,value):
 path=Path(path);fd,temp=tempfile.mkstemp(prefix='.selection-',dir=path.parent)
 try:
  with os.fdopen(fd,'w') as f:json.dump(value,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
  os.replace(temp,path)
  if os.name=='posix':
   fd=os.open(path.parent,os.O_RDONLY)
   try:os.fsync(fd)
   finally:os.close(fd)
 finally:
  if os.path.exists(temp):os.unlink(temp)
def signature(root,team):
 if sys.platform!='darwin':raise ValueError('Pinned Team ID verification requires macOS')
 app=root/'JoyDeck.app'
 subprocess.run(['/usr/bin/codesign','--verify','--deep','--strict',str(app)],check=True,capture_output=True)
 result=subprocess.run(['/usr/bin/codesign','-d','--verbose=4',str(app)],check=True,capture_output=True,text=True, encoding='utf-8')
 if 'TeamIdentifier='+team not in result.stderr.splitlines():raise ValueError('Signing team mismatch')
 return 'verified-team:'+team
def selection(root):
 path=Path(root)/'active.json'
 if not path.exists():return {'current':None,'previous':None}
 value=json.loads(path.read_text(encoding='utf-8'))
 for key in ('current','previous'):
  if value.get(key) is not None and not re.fullmatch('joydeck-[a-z0-9-]+',value[key]):raise ValueError('Invalid saved release pointer')
 return value
def install(archive,root,expected,allow_unsigned=False,team=None,activate=True):
 archive=Path(archive);root=Path(root).resolve()
 if not re.fullmatch('[0-9a-f]{64}',expected) or digest(archive)!=expected:raise ValueError('Archive does not match trusted SHA256')
 if not team and not allow_unsigned:raise ValueError('Supply pinned --team-id or explicit --allow-unsigned-development')
 with locked(root),tempfile.TemporaryDirectory(prefix='.stage-',dir=root) as temp:
  # Hash an owned copy, then extract exactly those bytes; never reopen a mutable external ZIP.
  owned=Path(temp)/'archive.zip'
  if archive.stat().st_size>256*1024*1024:raise ValueError('Archive budget exceeded')
  with archive.open('rb') as source,owned.open('xb') as target:shutil.copyfileobj(source,target,1024*1024)
  if digest(owned)!=expected:raise ValueError('Archive changed while staging')
  staged=extract(owned,temp);verified=verify(staged)
  sig=signature(staged,team) if team else 'unsigned-development-explicit'
  dest=root/staged.name
  if dest.exists():
   verify(dest)
   if (dest/'SHA256.json').read_bytes()!=(staged/'SHA256.json').read_bytes():raise ValueError('Existing release name contains different files')
  else:os.rename(staged,dest)
  old=selection(root)
  if activate and old.get('current')!=dest.name:atomic_json(root/'active.json',{'current':dest.name,'previous':old.get('current'),'archiveSha256':expected})
  return {'directory':str(dest),'activated':activate,'signature':sig,'filesVerified':verified['filesVerified'],'keptPrevious':old.get('current')}
def rollback(root):
 root=Path(root).resolve()
 with locked(root):
  old=selection(root);previous=old.get('previous')
  if not previous:raise ValueError('No previous release available')
  verify(root/previous)
  atomic_json(root/'active.json',{'current':previous,'previous':old['current']})
  return selection(root)
def main():
 p=argparse.ArgumentParser(description=__doc__);s=p.add_subparsers(dest='command',required=True)
 v=s.add_parser('verify');v.add_argument('directory',type=Path)
 i=s.add_parser('install');i.add_argument('archive',type=Path);i.add_argument('--root',type=Path,required=True);i.add_argument('--expected-sha256',required=True);i.add_argument('--team-id');i.add_argument('--allow-unsigned-development',action='store_true');i.add_argument('--stage-only',action='store_true')
 r=s.add_parser('rollback');r.add_argument('--root',type=Path,required=True)
 a=p.parse_args()
 try:
  result=verify(a.directory) if a.command=='verify' else rollback(a.root) if a.command=='rollback' else install(a.archive,a.root,a.expected_sha256,a.allow_unsigned_development,a.team_id,not a.stage_only)
  print(json.dumps(result,ensure_ascii=False,indent=2))
 except (OSError,ValueError,KeyError,subprocess.CalledProcessError,zipfile.BadZipFile) as e:print(str(e),file=sys.stderr);return 2
 return 0
if __name__=='__main__':raise SystemExit(main())
