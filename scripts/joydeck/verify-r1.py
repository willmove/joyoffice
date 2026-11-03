"""Exercise the actual headless executable against public-format fixtures and errors."""
import argparse
from pathlib import Path
import hashlib
import json
import shutil
import struct
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, default=ROOT / 'target/debug/joydeck-cli.exe')
    parser.add_argument('--evidence', type=Path, default=ROOT / 'docs/joydeck/evidence/R1/cli-verification.json')
    args = parser.parse_args()
    exe = args.cli.resolve()
    work = Path(tempfile.mkdtemp(prefix='joydeck-r1-cli-')) / '中文 空格路径'
    work.mkdir()
    records = []
    def run(arguments, expected=0):
        r = subprocess.run([str(exe), *map(str, arguments)], cwd=work, capture_output=True, encoding='utf-8')
        assert r.returncode == expected, (arguments, r.returncode, r.stderr)
        records.append({'args':list(map(str,arguments)), 'exit_code':r.returncode,'stdout':r.stdout,'stderr':r.stderr})
        return r.stdout
    for name in ['r1-preview.deckcraft', 'r1-preview.pptx', 'independent.pptx']:
        source = ROOT / 'tests/fixtures/joydeck' / name
        path = work / name
        shutil.copy2(source,path)
        before = hashlib.sha256(path.read_bytes()).hexdigest()
        info = json.loads(run(['inspect', path]))
        assert len(info['slides']) == 3 and info['read_only']
        output = work / (name+'.pngs')
        run(['render', path, '--output', output, '--all', '--scale', '1'])
        for image in sorted(output.glob('*.png')):
            data = image.read_bytes()
            assert data.startswith(b'\x89PNG\r\n\x1a\n')
            assert struct.unpack('>II',data[16:24]) == (960,540)
            assert len(data) > 2000, 'unexpectedly blank/small image'
        assert len(list(output.glob('*.png'))) == 3
        sums = {f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in output.iterdir()}
        run(['render',path,'--output',output],expected=1)
        assert sums == {f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in output.iterdir()}
        for flags in [['--page','0'],['--page','9999'],['--scale','NaN'],['--scale','8'],['--all','--page','1']]:
            run(['render',path,'--output',work/('error-'+str(len(records))),*flags],expected=1)
        assert before == hashlib.sha256(path.read_bytes()).hexdigest()
    broken = work/'损坏文件.pptx'; broken.write_bytes(b'PK broken')
    run(['inspect',broken],expected=1)
    future = work/'future.deckcraft'
    with zipfile.ZipFile(future,'w') as archive:
        archive.writestr('presentation.json',json.dumps({'format':'deckcraft','version':999}))
    run(['inspect',future],expected=1)
    run(['unknown-command'],expected=1)
    diagnostics = json.loads(run(['diagnostics']))
    assert {'pptx-read', 'deckcraft-read', 'render-png'}.issubset(diagnostics['capabilities']) and not diagnostics['cli_uses_gui']
    report={'cli':str(exe),'cli_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'working_directory':str(work),'checks':len(records),'passed':True,'runs':records}
    args.evidence.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(f'R1 executable verification passed: {len(records)} checks; {work}')

if __name__ == '__main__': main()
