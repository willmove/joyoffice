"""Verify the frozen final archive and run its GUI outside the repository."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import zipfile

OUT = Path(__file__).resolve().parent
package = json.loads((OUT/'package.json').read_text(encoding='utf-8'))
archive = Path(package['archive'])
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(archive) == package['archive_sha256']
work = Path(tempfile.mkdtemp(prefix='joyoffice-ui1-final-package-'))/'中文 空格'
work.mkdir()
with zipfile.ZipFile(archive) as z:
    assert z.testzip() is None
    assert all((work/n).resolve().is_relative_to(work.resolve()) for n in z.namelist())
    z.extractall(work)
installed = next(work.iterdir())
count = 0
for line in (installed/'SHA256SUMS').read_text(encoding='utf-8').splitlines():
    digest, name = line.split('  ', 1)
    assert sha(installed/name) == digest, name
    count += 1
candidate = Path(json.loads((OUT/'candidate-desktop-path.json').read_text(encoding='utf-8'))['path'])
assert sha(candidate) == json.loads((OUT/'candidate-desktop-public-read.json').read_text(encoding='utf-8'))['sha256']
shutil.copy2(candidate, OUT/'artifacts/candidate-formatted32.pptx')
source = work/'R9 UI1 键盘格式 原稿.pptx'
shutil.copy2(candidate, source)
env = os.environ.copy()
env['LOCALAPPDATA'] = str(work/'owned-state')
log = open(work/'runtime.txt', 'w', encoding='utf-8')
exe = installed/'joydeck.exe'
p = subprocess.Popen([str(exe), str(source), '--light', '--window-size=1440x900'], cwd=work, env=env, stdout=log, stderr=log)
report = dict(executable=str(exe), cli=str(installed/'joydeck-cli.exe'), pid=p.pid,
              work=str(work), source=str(source), source_sha256=sha(source),
              input_provenance='Owned candidate-formatting PPTX, independently verified; see candidate-desktop-public-read.json',
              gui_sha256=sha(exe), cli_sha256=sha(installed/'joydeck-cli.exe'),
              archive_sha256=sha(archive), payload_checks=count)
(OUT/'package-runtime.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
print(json.dumps(report, ensure_ascii=False))
