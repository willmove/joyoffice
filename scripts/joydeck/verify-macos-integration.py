"""Run a self-contained moved macOS .app; verify real file events and duplicate-open activation."""
import argparse,hashlib,importlib.util,json,os,shutil,subprocess,tempfile,time,zipfile
from pathlib import Path
from paths import ROOT, SCRIPTS, FIXTURES, EVIDENCE, LICENSES, binary, native_helper
spec=importlib.util.spec_from_file_location('bench',SCRIPTS / 'benchmark-m4.py');bench=importlib.util.module_from_spec(spec);spec.loader.exec_module(bench)
def main():
 p=argparse.ArgumentParser();p.add_argument('--app',type=Path,required=True);p.add_argument('--document',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();out.mkdir(parents=True,exist_ok=False)
 with tempfile.TemporaryDirectory(prefix='JoyDeck moved 测试 ') as tmp:
  moved=Path(tmp)/'移动的 JoyDeck.app';shutil.copytree(a.app.resolve(),moved);document=Path(tmp)/'文稿 有 空格😀.joydeck';shutil.copy2(a.document,document);binary=moved/'Contents/MacOS/JoyDeck'
  links=subprocess.check_output(['otool','-L',str(binary)],text=True, encoding='utf-8').splitlines()[1:]
  assert all(line.strip().startswith(('/usr/lib/','/System/Library/')) for line in links),links
  with (out/'native.log').open('wb') as log:
   # table-demo also isolates edit recovery files from the user's real recovery directory.
   proc=subprocess.Popen([str(binary),'--integration-smoke','--table-demo'],cwd=tmp,env=dict(os.environ,JOYDECK_STATE_DIR=str(out/'state')),stdout=log,stderr=subprocess.STDOUT)
   wake=subprocess.Popen(['caffeinate','-d','-i','-w',str(proc.pid)])
   try:
    def wait(predicate):
     deadline=time.monotonic()+15
     while time.monotonic()<deadline:
      assert proc.poll() is None,(out/'native.log').read_text(encoding='utf-8')
      if predicate():return
      time.sleep(.1)
     raise TimeoutError((out/'native.log').read_text(encoding='utf-8')[-2000:])
    wait(lambda:len(bench.windows(proc.pid))==1)
    subprocess.run(['open','-a',str(moved),str(document)],check=True)
    wait(lambda:(out/'native.log').read_text(encoding='utf-8').count('M13_OPEN_FILE accepted=true')>=1 and len(bench.windows(proc.pid))==2)
    time.sleep(1)
    subprocess.run(['open','-a',str(moved),str(document)],check=True)
    wait(lambda:(out/'native.log').read_text(encoding='utf-8').count('M13_OPEN_FILE accepted=true')>=2)
    time.sleep(.5);windows=bench.windows(proc.pid);assert len(windows)==2,windows
    selected=next((w for w in windows if '未命名文稿' not in w.get('kCGWindowName','')),None);assert selected,windows
    subprocess.run(['screencapture','-x','-o','-l',str(selected['kCGWindowNumber']),str(out/'file-open.png')],check=True)
    menus=subprocess.check_output(['osascript','-e',f'tell application "System Events" to tell (first application process whose unix id is {proc.pid}) to get name of every menu bar item of menu bar 1'],text=True, encoding='utf-8').strip()
    assert '文件' in menus and '编辑' in menus,menus
    def script(body):
     subprocess.run(['osascript','-e',f'tell application "System Events" to tell (first application process whose unix id is {proc.pid})\nset frontmost to true\n{body}\nend tell'],check=True,capture_output=True,text=True, encoding='utf-8')
    def page_count():
     try:
      with zipfile.ZipFile(document) as z:return len(json.loads(z.read('document.json'))['slides'])
     except (OSError,zipfile.BadZipFile):return -1
    original_pages=page_count();assert original_pages>0
    # Do not invoke File/Open first: menu history must work immediately after an OS file event.
    script('keystroke "n" using {command down, shift down}')
    time.sleep(.3);script('keystroke "s" using command down');wait(lambda:page_count()==original_pages+1)
    for label,expected in [('撤销',original_pages),('重做',original_pages+1),('撤销',original_pages)]:
     script(f'click menu item "{label}" of menu "编辑" of menu bar item "编辑" of menu bar 1')
     time.sleep(.3);script('keystroke "s" using command down');wait(lambda:page_count()==expected)
    subprocess.run(['screencapture','-x','-o','-l',str(selected['kCGWindowNumber']),str(out/'menu-history.png')],check=True)
    result={'binarySha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'appRelocated':True,'cwdOutsideRepository':True,'unicodeSpaceFileEvent':True,'sameFileActivatedExistingWindow':True,'windowsAfterDuplicateOpen':len(windows),'nativeMenus':menus,'menuUndoRedoBeforeOpenDialog':True,'savedPageCounts':[original_pages+1,original_pages,original_pages+1,original_pages],'systemDynamicLibrariesOnly':True,'cleanMachineInstall':False,'passed':True}
    (out/'verification.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n', encoding='utf-8');print(json.dumps(result,ensure_ascii=False,indent=2))
   finally:
    if proc.poll() is None:proc.terminate();proc.wait(timeout=8)
    wake.terminate();wake.wait(timeout=5)
    unregister=Path('/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister')
    if unregister.exists():subprocess.run([str(unregister),'-u',str(moved)],capture_output=True)
if __name__=='__main__':main()
