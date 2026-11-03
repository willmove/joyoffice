"""Verify and run isolated old-Deck/Word/Sheet development packages without installing them."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BACKUP = Path('D:/Coding/EditorProjects/a-backups/joyoffice-before-joydeck-r9-20261103-100321')


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def main():
    work = Path(tempfile.mkdtemp(prefix='joyoffice-r9-joint-packages-')) / '中文 空格'
    work.mkdir()
    records = []
    inputs = {'joyword': Path(json.loads((OUT / 'gui-word-output.json').read_text(encoding='utf-8'))['path']),
              'joysheet': Path(json.loads((OUT / 'gui-sheet-output.json').read_text(encoding='utf-8'))['path'])}
    for product, archive_path, wrapped in [
        ('joyword', ROOT / 'dist/joyword/joyword-0.1.1-windows-x86_64.zip', False),
        ('joysheet', ROOT / 'dist/joysheet/joysheet-0.1.0-r9-joint-windows-x86_64-debug.zip', True),
        ('old-joydeck', BACKUP / 'rollback-runtime/joydeck-pre-r9-a973f75-windows-amd64-debug.zip', True),
    ]:
        parent = work / product
        parent.mkdir()
        with zipfile.ZipFile(archive_path) as z:
            assert z.testzip() is None
            assert all((parent / n).resolve().is_relative_to(parent.resolve()) for n in z.namelist())
            z.extractall(parent)
        installed = next(parent.iterdir()) if wrapped else parent
        stem = 'joydeck' if product == 'old-joydeck' else product
        cli = installed / (stem + '-cli.exe')
        gui = installed / (stem + '.exe')
        build = json.loads((installed / 'BUILD-INFO.json').read_text())
        assert build['sourceSnapshotVerified'] and build['profile'] == 'debug'
        expected = {'joyword': '0.1.1', 'joysheet': '0.1.0', 'old-joydeck': '0.1.0'}[product]
        assert build['version'] == expected
        checks = None
        for name in ['PAYLOAD-SHA256.json', 'SHA256.json']:
            if (installed / name).is_file():
                hashes = json.loads((installed / name).read_text())
                for file, digest in hashes.items():
                    assert sha(installed / file) == digest, file
                checks = len(hashes)
        if product == 'old-joydeck':
            source = installed / 'fixtures/native-20.joydeck'
            assert sha(source) == sha(ROOT / 'tests/fixtures/joydeck/native-20.joydeck')
            command = [str(cli), '--read-root', str(source.parent), 'inspect', str(source)]
        else:
            source = work / inputs[product].name
            shutil.copy2(inputs[product], source)
            command = [str(cli), 'inspect' if product == 'joyword' else 'info', str(source)]
            if product == 'joysheet':
                command += ['--json']
        before = sha(source)
        result = subprocess.run(command, cwd=work, capture_output=True, text=True, encoding='utf-8', timeout=120)
        assert result.returncode == 0, (product, result.stdout, result.stderr)
        info = json.loads(result.stdout)
        assert 'error' not in info, info
        assert sha(source) == before
        log = OUT / (product + '-package-runtime.txt')
        env = os.environ.copy()
        if product == 'joyword':
            env['JOYWORD_RECOVERY_DIR'] = str(work / 'owned-word-recovery')
        with log.open('wb') as runtime:
            process = subprocess.Popen([str(gui), str(source)], cwd=work, env=env, stdout=runtime, stderr=subprocess.STDOUT)
        records.append(dict(product=product, archive=str(archive_path), archive_sha256=sha(archive_path),
                            installed=str(installed), executable=str(gui), gui_sha256=sha(gui),
                            cli_sha256=sha(cli), pid=process.pid, source=str(source), source_sha256=before,
                            payload_files_checked=checks, cli_result=info, stderr=result.stderr,
                            scope='debug package; native window verification recorded separately'))
    report = dict(work=str(work), passed=True, packages=records,
                  sheet_installer_sha256=sha(ROOT / 'dist/joysheet/joysheet-0.1.0-r9-joint-windows-x86_64-debug-setup.exe'),
                  installer_run=False)
    (OUT / 'other-packages.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(work=str(work), passed=True, products=[r['product'] for r in records]), ensure_ascii=False))


if __name__ == '__main__':
    main()
