"""Exercise a section that gains a page when a distant edit changes the width of its PAGE fields."""

import argparse
import json
from pathlib import Path
import re
import runpy
import secrets
import shutil
import sys
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request
from verify_save_changes import word_keys

SECTIONS = runpy.run_path(str(Path(__file__).with_name('check-section-reflow.py')))
CORPUS = runpy.run_path(str(Path(__file__).with_name('check-complex-corpus.py')))


def fixture(path, edited=False, facing_notes=False):
    def text(value):
        return f'<w:r><w:t xml:space="preserve">{value}</w:t></w:r>'

    def field(name):
        return f'<w:fldSimple w:instr="{name}">{text("99")}</w:fldSimple>'

    paragraphs = []
    for section in range(68):
        title = f'Section {section}'
        if edited and section == 34:
            title = 'Edited clause. ' * 740 + title
        props = '<w:pgNumType w:start="35"/>' if section == 0 else ''
        if section == 67:
            props = '<w:pgNumType w:start="7"/>'
        if facing_notes and section == 65:
            props += '<w:type w:val="evenPage"/><w:pgMar w:left="740" w:right="1220"/>'
        if section == 64:
            props += '<w:pgSz w:w="3680" w:h="1500"/><w:pgMar w:top="400" w:bottom="400" w:left="400" w:right="400"/>'
        end = f'<w:sectPr>{props}</w:sectPr>'
        spacing = '<w:spacing w:after="0"/>'
        paragraphs.append(f'<w:p><w:pPr>{spacing}</w:pPr>{text(title)}</w:p>')
        if section == 64:
            body = (field('PAGE') + text(' ')) * 12
        elif section in (2, 66, 67):
            body = text(f'R{section} Total=') + field('NUMPAGES') + text(' Page=') + field('PAGE')
        else:
            body = text('End')
        if facing_notes and section == 66:
            body += '<w:r><w:footnoteReference w:id="1"/></w:r>'
        paragraphs.append(f'<w:p><w:pPr>{spacing}{end if section < 67 else ""}</w:pPr>{body}</w:p>')
    paragraphs.append(end)
    styles = f'<w:styles xmlns:w="{CORPUS["W"]}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Source Sans 3" w:hAnsi="Source Sans 3"/></w:rPr></w:rPrDefault></w:docDefaults></w:styles>'
    rels = [('s', 'styles', 'styles.xml')]
    extras = {'word/styles.xml': styles}
    prefix = 'application/vnd.openxmlformats-officedocument.wordprocessingml.'
    types = {'word/styles.xml': prefix + 'styles+xml'}
    if facing_notes:
        note = ''.join(f'<w:p>{text(f"Continuation record {i:03}")}</w:p>' for i in range(58))
        extras['word/footnotes.xml'] = f'<w:footnotes xmlns:w="{CORPUS["W"]}"><w:footnote w:id="1">{note}</w:footnote></w:footnotes>'
        extras['word/settings.xml'] = f'<w:settings xmlns:w="{CORPUS["W"]}"><w:mirrorMargins/><w:evenAndOddHeaders/></w:settings>'
        types['word/settings.xml'] = prefix + 'settings+xml'
        rels.extend([('n', 'footnotes', 'footnotes.xml'), ('cfg', 'settings', 'settings.xml')])
    CORPUS['package'](path, ''.join(paragraphs), rels, extras, types)


def observed(pages):
    def title(section):
        return next(index for index, content in enumerate(pages, 1) if re.search(rf'\bSection\s+{section}\b', content))

    start, stop = title(64), title(65)
    fields = []
    for position in range(start, stop):
        content = re.sub(r'Section\s+64', '', pages[position - 1])
        numbers = [int(v) for v in re.findall(r'\b\d+\b', content)]
        assert all(value == position + 34 for value in numbers), (position, numbers)
        fields.extend(numbers)
    assert len(fields) == 12
    for section in (2, 66, 67):
        position = title(section)
        expected = 7 if section == 67 else position + 34
        assert f'R{section}Total={len(pages)}Page={expected}' in ''.join(pages[position - 1].split()), (section, pages[position - 1])
    note_pages = [i for i, page in enumerate(pages, 1) if 'Continuation record' in page]
    if note_pages:
        assert len(note_pages) >= 2 and sum(p.count('Continuation record') for p in pages) == 58
    return {'notePages': note_pages, 'pages': len(pages), 'editedPage': title(34), 'firstDirtyPage': title(2), 'fieldPages': list(range(start, stop)),
            'followingPage': title(66), 'restartPage': title(67), 'fieldNumbers': fields}


def run(desktop, old, current, folder, facing_notes=False):
    folder.mkdir(parents=True, exist_ok=False)
    original, working = folder / '原始.docx', folder / '分节 编辑.docx'
    fixture(original, facing_notes=facing_notes)
    fixture(folder / 'expanded.docx', edited=True, facing_notes=facing_notes)
    source_hash = digest(original)
    shutil.copyfile(original, working)
    reference = []
    for name, file in [('original', original), ('expanded', folder / 'expanded.docx')]:
        pdf = folder / (name + '.pdf')
        reference.append(observed(SECTIONS['printed_pages'](old, file, pdf)))
    before, after = reference
    assert len(before['fieldPages']) == 1 and len(after['fieldPages']) == (3 if facing_notes else 2)
    assert bool(before['notePages']) == bool(after['notePages']) == facing_notes
    n, key = port(), secrets.token_hex(32)
    args, env = ['--control', str(n), '--', str(working)], {'JOYWORD_CONTROL_KEY': key, 'JOYWORD_CONFIG_DIR': str(folder / 'config'), 'JOYWORD_PERF': '1'}
    log = folder / 'desktop.log'
    def inspect():
        return word_request(n, 'document.inspect', key=key).get('result')
    def command(action, **params):
        state = inspect()
        answer = word_request(n, 'engine.execute', {'doc': state['doc'], 'version': state['version'], 'command': action, 'params': params}, key)
        assert answer['ok'], answer
    def events(offset):
        with log.open() as f:
            f.seek(offset)
            return [json.loads(line.removeprefix('joyword-perf ')) for line in f if line.startswith('joyword-perf ') and line.endswith('}\n')]
    changed = 'Edited clause. ' * 740
    patches = []
    with app(desktop, args, folder, env) as proc:
        initial = wait(inspect, 'opened field document', proc)
        offset = 0
        def goto(page):
            command('edit.goto', page=page)
            wait(lambda: any(e.get('stage') == 'raster_installed' and e.get('page') == page for e in events(offset)), f'raster of field page {page}', proc)
        goto(before['editedPage'])
        expected_text = initial['text'].replace('Section 34', changed + 'Section 34', 1)
        for action, state in [('text.insert', after), ('edit.undo', before), ('edit.redo', after)]:
            offset = log.stat().st_size
            command(action, **({'text': changed} if action == 'text.insert' else {}))
            current_state = inspect()
            assert current_state['text'] == (initial['text'] if action == 'edit.undo' else expected_text)
            assert len(current_state['summary']['pages']) == state['pages']
            patch = wait(lambda: next((e for e in events(offset) if e.get('stage') == 'layout_patch'), None), 'bounded field reflow', proc)
            assert patch['firstPage'] == state['firstDirtyPage'] and patch['lastPage'] == max(v['pages'] for v in reference), patch
            visits = [state['firstDirtyPage'], *state['fieldPages'][:2], state['followingPage'], state['restartPage']]
            # The even-page break can leave a blank leaf following the field.
            # It has no caret target. Its raster is checked in the two exported PDFs.
            for page in visits:
                goto(page)
            goto(state['editedPage'])
            patches.append({'action': action, 'firstPage': patch['firstPage'], 'lastPage': patch['lastPage'], 'pages': state['pages'], 'displayedPages': visits})
        keys = word_keys(proc)
        for k in ('new', 'enter', 'enter'):
            keys(k)
        def saved():
            with ZipFile(working) as z:
                return changed in z.read('word/document.xml').decode()
        wait(saved, 'saved edited field document', proc)
    with app(desktop, args, folder, env, 'reopen.log') as proc:
        reopened = wait(inspect, 'reopened saved field document', proc)
        assert reopened['text'] == expected_text and len(reopened['summary']['pages']) == after['pages']
    pdfs = [folder / 'old.pdf', folder / 'new.pdf']
    for cli, pdf in zip((old, current), pdfs):
        assert observed(SECTIONS['printed_pages'](cli, working, pdf)) == after
    rasters = SECTIONS['compare_rasters'](*pdfs)
    assert len(rasters) == after['pages'] and digest(original) == source_hash
    return {
        'sourceSha256': source_hash, 'desktopSha256': digest(desktop),
        'original': before, 'expanded': after, 'nativePatches': patches, 'savedAndReopened': True,
        'twoVersionPageRastersSha256': rasters,
        'crossVersion': SECTIONS['verify_readers'](folder, old, current, after['pages']),
    }


if __name__ == '__main__':
    arg = argparse.ArgumentParser(description=__doc__)
    for key in ('desktop', 'baseline', 'candidate', 'output'):
        arg.add_argument(key, type=Path)
    arg.add_argument('--facing-notes', action='store_true', help='include mirrored sections, an even start, a numbering restart and a continued footnote')
    args = arg.parse_args()
    dest = args.output.resolve()
    result = run(args.desktop.resolve(), args.baseline.resolve(), args.candidate.resolve(), dest, args.facing_notes)
    (dest / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
