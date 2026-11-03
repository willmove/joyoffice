"""Reproduce R9 package, old-host corpus and native-save verification (read-only inputs)."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[4]
EVIDENCE = Path(__file__).resolve().parent


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--package', type=Path, required=True)
    args = ap.parse_args()
    package = args.package.resolve()
    work = Path(tempfile.mkdtemp(prefix='joyoffice-r9-package-')) / '中文 空格'
    work.mkdir()
    with zipfile.ZipFile(package) as archive:
        assert archive.testzip() is None
        for name in archive.namelist():
            assert (work / name).resolve().is_relative_to(work.resolve())
        archive.extractall(work)
    roots = list(work.iterdir())
    assert len(roots) == 1 and roots[0].is_dir()
    installed = roots[0]
    checksums = (installed / 'SHA256SUMS').read_text(encoding='utf-8').splitlines()
    for line in checksums:
        digest, relative = line.split('  ', 1)
        assert sha(installed / relative) == digest, relative
    build = json.loads((installed / 'BUILD-INFO.json').read_text())
    assert build['stage'] == 'R9' and build['sourceSnapshotVerified']
    assert build['packageRoots'] == ['joydeck', 'joydeck-cli']
    assert build['sourceGraphSha256'] == json.loads((installed / 'source.json').read_text())['sha256']
    assert (installed / 'SBOM.spdx.json').is_file()
    cli = installed / 'joydeck-cli.exe'
    runs = []

    def run(*cmd):
        result = subprocess.run([str(cli), *map(str, cmd)], cwd=work,
                                capture_output=True, text=True, encoding='utf-8', timeout=120)
        runs.append(dict(args=list(map(str, cmd)), exit_code=result.returncode,
                         stdout=result.stdout, stderr=result.stderr))
        assert result.returncode == 0, runs[-1]
        return result.stdout

    fixture = ROOT / 'tests/fixtures/joydeck'
    originals = {p.name: sha(p) for p in fixture.iterdir() if p.is_file()}
    migrated = work / '旧 JoyOffice 20 页 已迁移.joydeck'
    legacy = fixture / 'native-20.joydeck'
    run('migrate', legacy, '--output', migrated)
    inspected = json.loads(run('inspect', migrated))
    assert len(inspected['slides']) == 20
    with zipfile.ZipFile(migrated) as archive:
        assert archive.read('migration/legacy-original.joydeck') == legacy.read_bytes()
        migration_report = json.loads(archive.read('migration/report.json'))
        assert migration_report['source_ids']
    public = work / '迁移公共交换.pptx'
    run('exchange', migrated, '--mode', 'preserve', '--output', public)
    from pptx import Presentation
    assert len(Presentation(public).slides) == 20
    corpus = []
    for name in ['public-format-20.pptx', 'm8-powerpoint-edited.pptx',
                 'm9-powerpoint-edited-chart.pptx', 'm9-powerpoint-stale-cache.pptx',
                 'm10-powerpoint-authored-motion.pptx']:
        path = fixture / name
        info = json.loads(run('inspect', path))
        assert len(info['slides']) == len(Presentation(path).slides)
        out = work / (name + '.pngs')
        run('render', path, '--page', '1', '--output', out, '--scale', '0.5')
        images = list(out.glob('*.png'))
        assert len(images) == 1 and images[0].stat().st_size > 1000
        corpus.append(dict(path=str(path), sha256=sha(path), slides=len(info['slides']),
                           first_page_png_sha256=sha(images[0]), warnings=info.get('warnings')))
    assert originals == {p.name: sha(p) for p in fixture.iterdir() if p.is_file()}
    artifact = EVIDENCE / 'artifacts'
    artifact.mkdir(exist_ok=True)
    for p in [migrated, public]:
        shutil.copy2(p, artifact / p.name)

    word = json.loads((EVIDENCE / 'gui-word-output.json').read_text(encoding='utf-8'))
    sheet = json.loads((EVIDENCE / 'gui-sheet-output.json').read_text(encoding='utf-8'))
    wns = {'w': 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}

    def docx_text(path):
        with zipfile.ZipFile(path) as z:
            return ''.join(ET.fromstring(z.read('word/document.xml')).itertext())

    assert docx_text(word['path']) == docx_text(word['source'])
    assert 'R9 联合验证' not in docx_text(word['path'])
    with zipfile.ZipFile(sheet['path']) as z:
        ns = {'s': 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'}
        cell = ET.fromstring(z.read('xl/worksheets/sheet1.xml')).find(".//s:c[@r='B1']", ns)
        assert cell is not None
        assert cell.find('s:f', ns).text == '1+2'
        assert float(cell.find('s:v', ns).text) == 3
        charts = [n for n in z.namelist() if n.startswith('xl/charts/chart') and n.endswith('.xml')]
        assert len(charts) == 2
    saved = []
    for p in [Path(word['path']), Path(sheet['path'])]:
        dest = artifact / p.name
        shutil.copy2(p, dest)
        saved.append(dict(path=str(dest), sha256=sha(dest)))
    report = dict(passed=True, archive=str(package), archive_sha256=sha(package),
                  extracted=str(installed), sha256_entries_verified=len(checksums),
                  cli=str(cli), cli_sha256=sha(cli), gui_sha256=sha(installed / 'joydeck.exe'),
                  source_sha256=build['sourceGraphSha256'], corpus=corpus,
                  legacy_migration=dict(source_sha256=sha(legacy), slides=20,
                                        exact_original_attachment=True, report=migration_report),
                  native_saves=dict(word_text_matches_source=True, sheet_B1_formula='1+2',
                                    sheet_B1_value=3, sheet_charts=2, files=saved), runs=runs)
    (EVIDENCE / 'host-verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: report[k] for k in ['passed', 'extracted', 'sha256_entries_verified', 'cli_sha256', 'gui_sha256']}, ensure_ascii=False))


if __name__ == '__main__':
    main()
