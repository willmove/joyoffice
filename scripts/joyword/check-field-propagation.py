"""Exercise a section that gains a page when a distant edit changes the width of its PAGE fields."""

import argparse
import json
from pathlib import Path
import re
import runpy
import secrets
import shutil
import subprocess
import sys
from zipfile import ZipFile
import xml.etree.ElementTree as ET

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request
from verify_save_changes import word_keys

SECTIONS = runpy.run_path(str(Path(__file__).with_name('check-section-reflow.py')))
CORPUS = runpy.run_path(str(Path(__file__).with_name('check-complex-corpus.py')))


def fixture(path, edited=False, facing_notes=False, structured=False, column_geometry=False, numbered_lines=False, reviewed=False, separators=False):
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
        if column_geometry and section in (33, 35):
            props += ('<w:textDirection w:val="tbRl"/><w:vAlign w:val="center"/>' if section == 33
                      else '<w:textDirection w:val="btLr"/><w:vAlign w:val="bottom"/>')
        if column_geometry and section in (34, 36):
            props += ('<w:cols w:num="2" w:equalWidth="0" w:sep="1">'
                      '<w:col w:w="2680" w:space="520"/><w:col w:w="6160"/></w:cols>')
        if numbered_lines and 30 <= section <= 38:
            restart = 'newSection' if section == 36 else 'newPage' if section in (30, 38) else 'continuous'
            count = 3 if section == 36 else 1
            props += f'<w:lnNumType w:start="4" w:countBy="{count}" w:distance="360" w:restart="{restart}"/>'
        end = f'<w:sectPr>{props}</w:sectPr>'
        spacing = '<w:spacing w:after="0"/>'
        number = ''
        if structured and 8 <= section <= 43:
            number = f'<w:numPr><w:ilvl w:val="0"/><w:numId w:val="{1 if section < 35 else 2}"/></w:numPr>'
        suppress = '<w:suppressLineNumbers/>' if numbered_lines and section == 33 else ''
        paragraphs.append(f'<w:p><w:pPr>{number}{suppress}{spacing}</w:pPr>{text(title)}</w:p>')
        if section == 64:
            body = (field('PAGE') + text(' ')) * 12
        elif section in (2, 66, 67):
            body = text(f'R{section} Total=') + field('NUMPAGES') + text(' Page=') + field('PAGE')
        else:
            body = text('End')
        if facing_notes and section == 66:
            body += '<w:r><w:footnoteReference w:id="1"/></w:r>'
        if separators and section == 34:
            body += '<w:r><w:endnoteReference w:id="1"/></w:r>'
        if reviewed and section in (34, 62):
            cid = 0 if section == 34 else 1
            marked = (f'<w:commentRangeStart w:id="{cid}"/>' + text(f'AnchorWitness{cid}') +
                      f'<w:commentRangeEnd w:id="{cid}"/><w:r><w:commentReference w:id="{cid}"/></w:r>')
            if section == 34:
                marked += ('<w:del w:id="7" w:author="First reviewer" w:date="2020-01-01T00:00:00Z">'
                           '<w:r><w:delText>DeletionWitness</w:delText></w:r></w:del>'
                           '<w:ins w:id="8" w:author="Second reviewer" w:date="2020-01-02T00:00:00Z">'
                           + text('InsertionWitness') + '</w:ins>')
            paragraphs.append('<w:p>' + marked + '</w:p>')
        if column_geometry and section == 34:
            paragraphs.append(f'<w:p>{text("WestWitness")}<w:r><w:br w:type="column"/></w:r>{text("EastWitness")}</w:p>')
        if structured and section == 34:
            paragraphs.append(f'<w:p><w:pPr><w:framePr w:dropCap="drop" w:lines="3" w:hSpace="75"/></w:pPr>{text("C")}</w:p>')
            paragraphs.append(f'<w:p>{text("opper section caption with a raised initial.")}</w:p>')
            paragraphs.append(f'<w:p><w:pPr><w:framePr w:w="2180" w:y="60" w:hAnchor="text" w:vAnchor="text" w:wrap="around"/></w:pPr>{text("Framed annotation")}</w:p>')
        if structured and section in (2, 66):
            def cell(contents, width):
                return (f'<w:tbl><w:tblPr><w:tblW w:w="{width}" w:type="dxa"/></w:tblPr><w:tblGrid>'
                        f'<w:gridCol w:w="{width}"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="{width}" w:type="dxa"/>'
                        f'</w:tcPr>{contents}</w:tc></w:tr></w:tbl>')
            inner = cell(f'<w:p>{body}</w:p>', 3200)
            paragraphs.append(cell(inner + '<w:p/>', 3900))
            body = text('End')
        paragraphs.append(f'<w:p><w:pPr>{spacing}{end if section < 67 else ""}</w:pPr>{body}</w:p>')
    paragraphs.append(end)
    styles = f'<w:styles xmlns:w="{CORPUS["W"]}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Source Sans 3" w:hAnsi="Source Sans 3"/></w:rPr></w:rPrDefault></w:docDefaults></w:styles>'
    rels = [('s', 'styles', 'styles.xml')]
    extras = {'word/styles.xml': styles}
    prefix = 'application/vnd.openxmlformats-officedocument.wordprocessingml.'
    types = {'word/styles.xml': prefix + 'styles+xml'}
    if structured:
        extras['word/numbering.xml'] = (f'<w:numbering xmlns:w="{CORPUS["W"]}"><w:abstractNum w:abstractNumId="0">'
            '<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/>'
            '<w:suff w:val="space"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>'
            '<w:num w:numId="2"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="9"/>'
            '</w:lvlOverride></w:num></w:numbering>')
        rels.append(('num', 'numbering', 'numbering.xml'))
        types['word/numbering.xml'] = prefix + 'numbering+xml'
    if reviewed:
        contents = ''.join(f'<w:comment w:id="{i}" w:author="Reviewer{i}" w:initials="R{i}" '
                           f'w:date="2020-01-01T00:00:00Z"><w:p>{text(f"BalloonWitness{i}")}</w:p></w:comment>'
                           for i in range(2))
        extras['word/comments.xml'] = f'<w:comments xmlns:w="{CORPUS["W"]}">{contents}</w:comments>'
        rels.append(('review', 'comments', 'comments.xml'))
    if facing_notes:
        note = ''.join(f'<w:p>{text(f"Continuation record {i:03}")}</w:p>' for i in range(58))
        extras['word/footnotes.xml'] = f'<w:footnotes xmlns:w="{CORPUS["W"]}"><w:footnote w:id="1">{note}</w:footnote></w:footnotes>'
        extras['word/settings.xml'] = f'<w:settings xmlns:w="{CORPUS["W"]}"><w:mirrorMargins/><w:evenAndOddHeaders/></w:settings>'
        types['word/settings.xml'] = prefix + 'settings+xml'
        rels.extend([('n', 'footnotes', 'footnotes.xml'), ('cfg', 'settings', 'settings.xml')])
    if separators:
        for kind in ('footnote', 'endnote'):
            special = ''.join(
                f'<w:{kind} w:type="{role}" w:id="{ident}"><w:p>{text(kind + witness)}{rule}</w:p></w:{kind}>'
                for role, ident, witness, rule in [
                    ('separator', -1, 'BeginWitness', '<w:r><w:separator/></w:r>'),
                    ('continuationSeparator', 0, 'ResumeWitness', '<w:r><w:continuationSeparator/></w:r>'),
                    ('continuationNotice', -2, 'NextWitness', '')])
            part = f'word/{kind}s.xml'
            if kind == 'endnote':
                extras[part] = (f'<w:endnotes xmlns:w="{CORPUS["W"]}"><w:endnote w:id="1"><w:p>'
                                + text('EndnoteBodyWitness') + '</w:p></w:endnote></w:endnotes>')
                rels.append(('en', 'endnotes', 'endnotes.xml'))
            extras[part] = extras[part].replace(f'</w:{kind}s>', special + f'</w:{kind}s>')

    CORPUS['package'](path, ''.join(paragraphs), rels, extras, types)


def observed(pages, structured=False):
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
    labels = {}
    if structured:
        for section in (*range(8, 34), *range(35, 44)):
            expected = section - 7 if section < 35 else section - 26
            content = ' '.join(pages[title(section) - 1].split())
            assert re.search(rf'\b{expected}\.\s*Section {section}\b', content), (section, expected, content)
            labels[section] = expected
        assert any('Framed annotation' in text for text in pages)
    return {**({'listNumbers': labels} if structured else {}), 'notePages': note_pages, 'pages': len(pages), 'editedPage': title(34), 'firstDirtyPage': title(2), 'fieldPages': list(range(start, stop)),
            'followingPage': title(66), 'restartPage': title(67), 'fieldNumbers': fields}


def structured_content(document):
    ns = {'w': CORPUS['W']}
    with ZipFile(document) as z:
        doc = ET.fromstring(z.read('word/document.xml'))
        numbers = ET.fromstring(z.read('word/numbering.xml'))
    frames = doc.findall('.//w:framePr', ns)
    caps = [f for f in frames if f.get('{%s}dropCap' % ns['w']) == 'drop']
    summary = {
        'numberedParagraphs': len(doc.findall('.//w:numPr', ns)),
        'numberRestartsAtNine': sum(n.get('{%s}val' % ns['w']) == '9' for n in numbers.findall('.//w:startOverride', ns)),
        'nestedTables': len(doc.findall('.//w:tc/w:tbl', ns)),
        'threeLineDropCaps': sum(f.get('{%s}lines' % ns['w']) == '3' for f in caps),
        'textFrames': len(frames) - len(caps),
    }
    assert summary == dict(numberedParagraphs=36, numberRestartsAtNine=1, nestedTables=2, threeLineDropCaps=1, textFrames=1), summary
    return summary


def section_geometry(document):
    ns = {'w': CORPUS['W']}
    name = lambda key: '{%s}%s' % (ns['w'], key)
    with ZipFile(document) as archive:
        root = ET.fromstring(archive.read('word/document.xml'))
    columns = []
    for group in root.findall('.//w:cols/w:col/..', ns):
        if group.get(name('equalWidth')) == '0' and group.get(name('sep')) in ('1', 'true'):
            columns.append([(int(col.get(name('w'))), int(col.get(name('space'), '0'))) for col in group.findall('w:col', ns)])
    turns = [value.get(name('val')) for value in root.findall('.//w:sectPr/w:textDirection', ns)]
    align = [value.get(name('val')) for value in root.findall('.//w:sectPr/w:vAlign', ns)]
    breaks = sum(element.get(name('type')) == 'column' for element in root.findall('.//w:br', ns))
    assert columns == [[(2680, 520), (6160, 0)]] * 2, columns
    assert turns == ['tbRl', 'btLr'] and align == ['center', 'bottom'] and breaks == 1, (turns, align, breaks)
    return {'columnsTwips': columns, 'directions': turns, 'verticalAlignments': align, 'columnBreaks': breaks}


def line_number_settings(document):
    ns = {'w': CORPUS['W']}
    with ZipFile(document) as archive:
        root = ET.fromstring(archive.read('word/document.xml'))
    def values(node):
        return {key: node.get(f'{{{ns["w"]}}}{key}') for key in ('start', 'countBy', 'distance', 'restart')}
    settings = [values(n) for n in root.findall('.//w:sectPr/w:lnNumType', ns)]
    assert len(settings) == 9 and all(n['start'] == '4' and n['distance'] == '360' for n in settings), settings
    assert [n['restart'] for n in settings] == ['newPage'] + ['continuous'] * 5 + ['newSection', 'continuous', 'newPage']
    assert [n['countBy'] for n in settings] == ['1'] * 6 + ['3', '1', '1']
    assert len(root.findall('.//w:suppressLineNumbers', ns)) == 1
    return settings


def margin_labels(pdf):
    root = ET.fromstring(subprocess.check_output(['pdftotext', '-bbox', str(pdf), '-']))
    result = {}
    for i, page in enumerate(root.findall('.//{*}page'), 1):
        labels = [int(word.text) for word in page.findall('.//{*}word')
                  if word.text and word.text.isdecimal() and float(word.get('xMin')) < 0.1 * float(page.get('width'))]
        if labels:
            result[i] = labels
    return result


def column_witnesses(pdf):
    root = ET.fromstring(subprocess.check_output(['pdftotext', '-bbox', str(pdf), '-']))
    words = {}
    for i, page in enumerate(root.findall('.//{*}page'), 1):
        for word in page.findall('.//{*}word'):
            if word.text in ('WestWitness', 'EastWitness'):
                assert word.text not in words
                words[word.text] = [i, float(word.get('xMin')), float(word.get('yMin'))]
    assert len(words) == 2, words
    return words


def reviewed_content(path):
    ns = {'w': CORPUS['W']}
    with ZipFile(path) as archive:
        main = ET.fromstring(archive.read('word/document.xml'))
        comments = ET.fromstring(archive.read('word/comments.xml'))
    attr = '{' + ns['w'] + '}'
    tags = {kind: [node.get(attr + 'id') for node in main.findall('.//w:' + kind, ns)]
            for kind in ('commentRangeStart', 'commentRangeEnd', 'commentReference')}
    anchors = set(tags['commentRangeStart'])
    assert len(anchors) == 2 and all(set(ids) == anchors for ids in tags.values()), tags
    texts = {node.get(attr + 'id'): ''.join(node.itertext()).strip() for node in comments.findall('w:comment', ns)}
    assert set(texts) == anchors and set(texts.values()) == {'BalloonWitness0','BalloonWitness1'}, texts
    revisions = {kind: [(n.get(attr + 'author'), ''.join(n.itertext()).strip()) for n in main.findall('.//w:' + kind, ns)]
                 for kind in ('ins', 'del')}
    assert revisions == {'ins': [('Second reviewer', 'InsertionWitness')], 'del': [('First reviewer', 'DeletionWitness')]}, revisions
    return {'comments': sorted(texts.values()), 'anchorTriples': len(anchors), 'revisions': revisions}


def print_review(cli, document, pdf):
    requests = [('open_document', {'path': str(document)}),
                ('execute', {'command': 'file.exportPdf', 'params': {'path': str(pdf), 'markup': True}})]
    lines = [json.dumps({'jsonrpc': '2.0', 'id': i, 'method': 'tools/call',
                         'params': {'name': tool, 'arguments': args}}) for i, (tool, args) in enumerate(requests, 1)]
    result = subprocess.run([cli, 'mcp', '--automation-read-root', document.parent, '--automation-write-root', pdf.parent],
                            input='\n'.join(lines) + '\n', capture_output=True, text=True, check=True, timeout=60)
    replies = [json.loads(line)['result'] for line in result.stdout.splitlines()]
    assert len(replies) == 2 and all(not r.get('isError') for r in replies), replies
    pages = subprocess.check_output(['pdftotext', '-layout', pdf, '-'], text=True).split('\f')
    witnesses = {}
    for word in ('BalloonWitness0', 'BalloonWitness1', 'AnchorWitness0', 'AnchorWitness1', 'DeletionWitness', 'InsertionWitness'):
        (page,) = [number for number, content in enumerate(pages, 1) if word in content]
        witnesses[word] = page
    assert witnesses['BalloonWitness0'] == witnesses['AnchorWitness0']
    assert witnesses['BalloonWitness1'] == witnesses['AnchorWitness1']
    return witnesses


def run(desktop, old, current, folder, facing_notes=False, structured=False, column_geometry=False, numbered_lines=False, reviewed=False, separators=False):
    folder.mkdir(parents=True, exist_ok=False)
    original, working = folder / '原始.docx', folder / '分节 编辑.docx'
    fixture(original, facing_notes=facing_notes, structured=structured, column_geometry=column_geometry, numbered_lines=numbered_lines, reviewed=reviewed, separators=separators)
    fixture(folder / 'expanded.docx', edited=True, facing_notes=facing_notes, structured=structured, column_geometry=column_geometry, numbered_lines=numbered_lines, reviewed=reviewed, separators=separators)
    source_hash = digest(original)
    shutil.copyfile(original, working)
    reference = []
    for name, file in [('original', original), ('expanded', folder / 'expanded.docx')]:
        pdf = folder / (name + '.pdf')
        reference.append(observed(SECTIONS['printed_pages'](old, file, pdf), structured))
    before, after = reference
    assert len(before['fieldPages']) == 1 and len(after['fieldPages']) == (3 if facing_notes else 2)
    assert bool(before['notePages']) == bool(after['notePages']) == facing_notes
    n, key = port(), secrets.token_hex(32)
    args, env = ['--control', str(n), '--', str(working)], {'JOYWORD_CONTROL_KEY': key, 'JOYWORD_CONFIG_DIR': str(folder / 'config'), 'JOYWORD_PERF': '1'}
    log = folder / 'desktop.log'
    def inspect():
        return word_request(n, 'document.inspect', key=key).get('result')
    def command(action, **params):
        state = wait(inspect, "current document revision", proc)
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
        assert observed(SECTIONS['printed_pages'](cli, working, pdf), structured) == after
    rasters = SECTIONS['compare_rasters'](*pdfs)
    assert len(rasters) == after['pages'] and digest(original) == source_hash
    exchange = SECTIONS['verify_readers'](folder, old, current, after['pages'])
    formats = ({'structuredContentByWriter': {name: structured_content(folder / file) for name, file in
                [('original', original.name), ('native', working.name), ('baseline', 'baseline-resaved.docx'),
                 ('candidate', 'candidate-resaved.docx')]}} if structured else {})
    if column_geometry:
        formats['geometryByWriter'] = {name: section_geometry(file) for name, file in
            [('source', original), ('native', working), ('baseline', folder / 'baseline-resaved.docx'),
             ('candidate', folder / 'candidate-resaved.docx')]}
        witnesses = {key: column_witnesses(folder / (key + '.pdf')) for key in ('original', 'expanded', 'old', 'new')}
        west, east = (witnesses['original'][k] for k in ('WestWitness', 'EastWitness'))
        assert west[0] == east[0] and abs(east[1] - west[1] - 160.) < .5, witnesses
        assert witnesses['expanded'] == witnesses['old'] == witnesses['new'], witnesses
        formats['pdfColumnWitnesses'] = witnesses
    if numbered_lines:
        formats['lineNumberSettingsByWriter'] = {name: line_number_settings(file) for name, file in
            [('source', original), ('native', working), ('baseline', folder / 'baseline-resaved.docx'),
             ('candidate', folder / 'candidate-resaved.docx')]}
        labels = {key: margin_labels(folder / (key + '.pdf')) for key in ('original', 'expanded', 'old', 'new')}
        assert labels['expanded'] == labels['old'] == labels['new']
        for key in ('original', 'expanded'):
            values = list(labels[key].values())
            assert values[:4] == [[5, 6], [7, 8], [9, 10], [11]], values[:4]
            assert values[-3:] == [[6], [7, 8], [5, 6]], values[-3:]
            continuous = [n for page in values[:-3] for n in page]
            assert continuous == list(range(5, 5 + len(continuous)))
        assert max(labels['expanded']) > max(labels['original']) and len(labels['expanded']) > len(labels['original'])
        formats['marginLabels'] = {k: labels[k] for k in ('original', 'expanded')}
    if separators:
        def note_entries(filename):
            with ZipFile(filename) as archive:
                roles = {}
                for kind in ('footnote', 'endnote'):
                    root = ET.fromstring(archive.read(f'word/{kind}s.xml'))
                    for element in root:
                        role = element.get('{%s}type' % CORPUS['W'])
                        if role in ('separator', 'continuationSeparator', 'continuationNotice'):
                            roles[(kind + '/' + role)] = ''.join(n.text or '' for n in element.findall('.//{*}t'))
                assert set(roles.values()) == {kind + tag for kind in ('footnote', 'endnote')
                                               for tag in ('BeginWitness', 'ResumeWitness', 'NextWitness')}, roles
                return roles
        files = {'original': original, 'native': working, 'previous': folder / 'baseline-resaved.docx',
                 'current': folder / 'candidate-resaved.docx'}
        formats['separatorStories'] = {key: note_entries(file) for key, file in files.items()}
        def locations(pdf):
            pages = subprocess.check_output(['pdftotext', '-layout', pdf, '-'], text=True).split('\f')
            tags = ['footnoteBeginWitness', 'footnoteResumeWitness', 'footnoteNextWitness',
                    'endnoteBeginWitness', 'EndnoteBodyWitness']
            result = {tag: [i for i, page in enumerate(pages, 1) if tag in ''.join(page.split())] for tag in tags}
            assert all(result.values()), result
            return result
        loc = {key: locations(folder / (key + '.pdf')) for key in ('original', 'expanded', 'old', 'new')}
        assert loc['expanded'] == loc['old'] == loc['new'], loc
        formats['separatorPages'] = loc
    if reviewed:
        formats['reviewByWriter'] = {name: reviewed_content(file) for name, file in
            [('source', original), ('native', working), ('baseline', folder / 'baseline-resaved.docx'),
             ('candidate', folder / 'candidate-resaved.docx')]}
        old_pdf, new_pdf = folder / 'old-review.pdf', folder / 'new-review.pdf'
        prior, accepted = print_review(old, working, old_pdf), print_review(current, working, new_pdf)
        assert prior == accepted
        review_pages = SECTIONS['compare_rasters'](old_pdf, new_pdf)
        assert len(review_pages) == after['pages']
        formats['printedMarkup'] = {'witnessPages': accepted, 'comparedRasters': len(review_pages)}
    return {
        **formats, 'sourceSha256': source_hash, 'desktopSha256': digest(desktop),
        'original': before, 'expanded': after, 'nativePatches': patches, 'savedAndReopened': True,
        'twoVersionPageRastersSha256': rasters,
        'crossVersion': exchange,
    }


if __name__ == '__main__':
    arg = argparse.ArgumentParser(description=__doc__)
    for key in ('desktop', 'baseline', 'candidate', 'output'):
        arg.add_argument(key, type=Path)
    arg.add_argument('--facing-notes', action='store_true', help='include mirrored sections, an even start, a numbering restart and a continued footnote')
    arg.add_argument('--structured', action='store_true', help='exercise continued and restarted lists, a frame, a drop cap and nested table fields')
    arg.add_argument('--column-geometry', action='store_true', help='exercise unequal columns, an explicit column break and both rotated text directions')
    arg.add_argument('--numbered-lines', action='store_true', help='exercise per-page, section and continuous margin line numbering')
    arg.add_argument('--separators', action='store_true', help='exercise custom footnotes, continuation notices and an endnote separator')
    arg.add_argument('--reviewed', action='store_true', help='include tracked revisions, anchored comments and independent printed markup')
    args = arg.parse_args()
    dest = args.output.resolve()
    result = run(args.desktop.resolve(), args.baseline.resolve(), args.candidate.resolve(), dest, args.facing_notes or args.separators, args.structured, args.column_geometry, args.numbered_lines, args.reviewed, args.separators)
    (dest / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
