"""R3 executable verification with public PPTX XML and independent python-pptx readback."""
from pathlib import Path
import argparse, hashlib, json, queue, struct, subprocess, sys, tempfile, threading, zipfile

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, default=ROOT/'target/debug/joydeck-cli.exe')
    parser.add_argument('--evidence', type=Path, default=ROOT/'docs/joydeck/evidence/R3/cli-verification.json')
    args = parser.parse_args()
    exe = args.cli.resolve()
    sys.path.insert(0, str(ROOT/'target/corpus-tools'))
    from pptx import Presentation
    work = Path(tempfile.mkdtemp(prefix='joydeck-r3-'))/'中文 空格路径'
    work.mkdir()
    source = work/'独立 原稿.pptx'
    with zipfile.ZipFile(ROOT/'tests/fixtures/joydeck/independent.pptx') as src, zipfile.ZipFile(source, 'w', zipfile.ZIP_DEFLATED) as out:
        for name in src.namelist():
            out.writestr(name, src.read(name))
        out.writestr('vendor/r3-opaque.xml', b'<future xmlns="urn:r3">keep exact bytes</future>')
    initial = hashlib.sha256(source.read_bytes()).hexdigest()
    records = []

    def run(arguments, expected=0):
        r = subprocess.run([str(exe), *map(str, arguments)], cwd=work, capture_output=True, encoding='utf-8', timeout=45)
        records.append(dict(args=list(map(str, arguments)), exit_code=r.returncode, stdout=r.stdout, stderr=r.stderr))
        assert r.returncode == expected, (arguments, r.returncode, r.stderr)
        return r.stdout

    def edit(script, output, expected=0, input_path=source):
        path = work/('script-'+str(len(records))+'.json')
        path.write_text(json.dumps(script, ensure_ascii=False), encoding='utf-8')
        return run(['edit', input_path, '--script', path, '--output', output], expected)

    info = json.loads(run(['inspect', source]))
    title = info['slides'][0]['objects'][0]['id']
    table = next(o['id'] for o in info['slides'][1]['objects'] if o['kind'] == 'table')
    text = 'JoyDeck R3 中文😀 e\u0301 / עברית'
    notes = 'R3 notes / 中文备注'
    body = {'paragraphs': [{'runs': [{'text': text, 'props': {'bold': True, 'italic': True}}]}]}
    script = [dict(command='set-text', target=dict(kind='shape', shape=title), body=body),
              dict(command='set-text', target=dict(kind='notes'), body={'paragraphs': [{'runs': [{'text': notes}]}]}),
              dict(command='select', page=2, shapes=[table]),
              dict(command='table', action=dict(operation='paste-tsv', row=0, col=0, text='中文😀\tB\nC\tD')),
              dict(command='table', action=dict(operation='fill', row=0, col=0, rgb=0xffe8a0)),
              dict(command='table', action=dict(operation='merge', row=1, col=0, end_row=1, end_col=1)),
              dict(command='undo'), dict(command='redo')]
    outputs = []
    for extension in ['pptx', 'joydeck']:
        output = work/('R3编辑.'+extension)
        edit(script, output)
        result = json.loads(run(['inspect', output]))
        assert result['slides'][0]['objects'][0]['text'] == text
        assert result['slides'][0]['notes'] == notes
        t = next(o['table'] for o in result['slides'][1]['objects'] if o['id'] == table)
        assert t['rows'][0]['cells'][0]['text']['paragraphs'][0]['runs'][0]['text'] == '中文😀'
        assert t['rows'][1]['cells'][0]['gridSpan'] == 2 and t['rows'][1]['cells'][1]['hMerge']
        edit(script, output, 1)
        run(['render', output, '--output', work/('render-'+extension), '--all'])
        for png in (work/('render-'+extension)).glob('*.png'):
            assert struct.unpack('>II', png.read_bytes()[16:24]) == (960, 540)
        outputs.append(output)
    p = Presentation(outputs[0])
    assert p.slides[0].shapes[0].text == text
    assert p.slides[0].shapes[0].text_frame.paragraphs[0].runs[0].font.bold
    assert p.slides[0].shapes[0].text_frame.paragraphs[0].runs[0].font.italic
    assert notes in p.slides[0].notes_slide.notes_text_frame.text
    t = next(s.table for s in p.slides[1].shapes if s.has_table)
    assert t.cell(0, 0).text == '中文😀' and t.cell(1, 0).is_merge_origin
    with zipfile.ZipFile(source) as a, zipfile.ZipFile(outputs[0]) as b:
        touched = {'ppt/slides/slide1.xml', 'ppt/slides/slide2.xml', 'ppt/notesSlides/notesSlide1.xml'}
        for name in a.namelist():
            if name not in touched:
                assert a.read(name) == b.read(name), name
    converted = work/'native-to-public.pptx'
    edit([], converted, input_path=outputs[1])
    assert Presentation(converted).slides[0].shapes[0].text == text
    structure = work/'structure.pptx'
    edit([dict(command='select', page=2, shapes=[table]), dict(command='table', action=dict(operation='insert-row', at=1)),
          dict(command='table', action=dict(operation='insert-col', at=1))], structure)
    t = next(s.table for s in Presentation(structure).slides[1].shapes if s.has_table)
    assert len(t.rows) == 4 and len(t.columns) == 3
    unknown = work/'table-unknown.pptx'
    with zipfile.ZipFile(source) as a, zipfile.ZipFile(unknown, 'w', zipfile.ZIP_DEFLATED) as b:
        for name in a.namelist():
            data = a.read(name)
            if name == 'ppt/slides/slide2.xml':
                assert b'<a:tcPr' in data
                data = data.replace(b'<a:tcPr', b'<a:tcPr future-r3="keep"', 1)
            b.writestr(name, data)
    cell_script = [dict(command='select', page=2, shapes=[table]), dict(command='edit-text', target=dict(kind='cell', shape=table, row=0, col=0), start=0, end=0, text='保留 ')]
    preserved = work/'unknown-cell-edited.pptx'
    edit(cell_script, preserved, input_path=unknown)
    with zipfile.ZipFile(preserved) as z:
        assert b'future-r3="keep"' in z.read('ppt/slides/slide2.xml')
    structural_script = [dict(command='select', page=2, shapes=[table]), dict(command='table', action=dict(operation='insert-row', at=1))]
    rejected = work/'must-not-exist.pptx'
    edit(structural_script, rejected, 1, unknown)
    assert not rejected.exists()
    edit(structural_script, work/'unknown-preserved.joydeck', input_path=unknown)
    # One successful mutation followed by invalid UTF-16 must not publish partial output.
    invalid = work/'invalid-range.pptx'
    edit([dict(command='set-text', target=dict(kind='shape', shape=title), body={'paragraphs': [{'runs': [{'text': '😀'}]}]}),
          dict(command='edit-text', target=dict(kind='shape', shape=title), start=1, end=2, text='bad')], invalid, 1)
    assert not invalid.exists()
    recovery = work/'recovery'
    batch = work/'recovery-batch.json'
    batch.write_text(json.dumps(script, ensure_ascii=False), encoding='utf-8')
    ready = queue.Queue()
    child = subprocess.Popen([str(exe), 'checkpoint-probe', str(source), str(batch), str(recovery)], cwd=work, stdout=subprocess.PIPE, stderr=subprocess.PIPE, encoding='utf-8')
    threading.Thread(target=lambda: ready.put(child.stdout.readline()), daemon=True).start()
    try:
        line = ready.get(timeout=20)
        assert line.startswith('READY '), line
        session = line.strip().split(' ', 1)[1]
        assert json.loads(run(['recovery-list', recovery]))[0]['active']
    finally:
        child.kill()
        child.wait(timeout=5)
    rows = json.loads(run(['recovery-list', recovery]))
    assert any(r['record']['session'] == session and not r['active'] and r['problem'] is None for r in rows)
    restored = work/'recovered-r3.joydeck'
    run(['recover', recovery, session, '--output', restored])
    recovered = json.loads(run(['inspect', restored]))
    assert recovered['slides'][0]['notes'] == notes and recovered['slides'][0]['objects'][0]['text'] == text
    recovered_pptx = work/'recovered-r3.pptx'
    edit([], recovered_pptx, input_path=restored)
    assert next(s.table for s in Presentation(recovered_pptx).slides[1].shapes if s.has_table).cell(0, 0).text == '中文😀'
    assert hashlib.sha256(source.read_bytes()).hexdigest() == initial
    report = dict(work=str(work), cli=str(exe), cli_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),
                  independent_reader='python-pptx 1.0.2', checks=len(records), passed=True,
                  source_unchanged=True, untouched_zip_members_preserved=True,
                  unknown_cell_xml_preserved=True, unsafe_structure_rejected=True,
                  forced_process_recovery=True, recovered_rich_text_table_notes=True, records=records)
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(f'R3 public/native/table/text/notes checks passed: {len(records)} executable calls')


if __name__ == '__main__':
    main()
