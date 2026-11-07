#!/usr/bin/env python3
"""Generate synthetic notes/math/OLE/chart/SmartArt DOCX and compare two trusted offline CLIs.

No OLE payload is executed. These are package-preservation fixtures, not Office
rendering or real embedded-application qualification. Output must be new.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import posixpath
import subprocess
import xml.etree.ElementTree as ET
import zipfile

W = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
M = 'http://schemas.openxmlformats.org/officeDocument/2006/math'
NS = f'xmlns:w="{W}" xmlns:r="{R}" xmlns:m="{M}" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"'
PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jL1sAAAAASUVORK5CYII=')
OLE = b'SYNTHETIC-INERT-OLE-PRESERVATION-PAYLOAD\x00\xff'


def require(ok, message):
    if not ok:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relationships(items):
    return '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">' + ''.join(
        f'<Relationship Id="{rid}" Type="{R}/{kind}" Target="{target}"/>' for rid, kind, target in items) + '</Relationships>'


def package(path, body, rels=(), extra=None, types=None):
    parts = {'_rels/.rels': relationships([('root', 'officeDocument', 'word/document.xml')]),
             'word/document.xml': f'<w:document {NS}><w:body>{body}</w:body></w:document>',
             'word/_rels/document.xml.rels': relationships(rels), **(extra or {})}
    content = {'document': 'document.main', 'footnotes': 'footnotes', 'endnotes': 'endnotes', 'comments': 'comments'}
    parts['[Content_Types].xml'] = '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="bin" ContentType="application/vnd.openxmlformats-officedocument.oleObject"/>' + ''.join(
        f'<Override PartName="/word/{name}.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.{kind}+xml"/>' for name, kind in content.items() if f'word/{name}.xml' in parts) + ''.join(f'<Override PartName="/{name}" ContentType="{kind}"/>' for name, kind in (types or {}).items()) + '</Types>'
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name, value in sorted(parts.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            archive.writestr(info, value.encode() if isinstance(value, str) else value)


def generate(out):
    extra = {}
    for kind, text in [('footnote', 'Synthetic footnote'), ('endnote', 'Synthetic endnote')]:
        extra[f'word/{kind}s.xml'] = f'<w:{kind}s {NS}><w:{kind} w:id="1"><w:p><w:r><w:{kind}Ref/><w:t>{text}</w:t></w:r></w:p></w:{kind}></w:{kind}s>'
    package(out / 'notes.docx', '<w:p><w:r><w:t>Notes body</w:t><w:footnoteReference w:id="1"/><w:endnoteReference w:id="1"/></w:r></w:p>', [('f', 'footnotes', 'footnotes.xml'), ('e', 'endnotes', 'endnotes.xml')], extra)
    package(out / 'equation.docx', '<w:p><m:oMath><m:f><m:num><m:r><m:t>x+1</m:t></m:r></m:num><m:den><m:r><m:t>y</m:t></m:r></m:den></m:f></m:oMath></w:p>')
    package(out / 'ole.docx', '<w:p><w:r><w:object w:dxaOrig="1440" w:dyaOrig="720"><v:shape id="shape1" type="#_x0000_t75" style="width:72pt;height:36pt" o:ole=""><v:imagedata r:id="image"/></v:shape><o:OLEObject Type="Embed" ProgID="Package" ShapeID="shape1" DrawAspect="Content" ObjectID="_1000000001" r:id="ole"/></w:object></w:r></w:p>', [('image', 'image', 'media/preview.png'), ('ole', 'oleObject', 'embeddings/payload.bin')], {'word/media/preview.png': PNG, 'word/embeddings/payload.bin': OLE})

    a = 'http://schemas.openxmlformats.org/drawingml/2006/main'
    c = 'http://schemas.openxmlformats.org/drawingml/2006/chart'
    d = 'http://schemas.openxmlformats.org/drawingml/2006/diagram'
    def drawing(uri, payload):
        return f'<w:p><w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="{a}"><wp:extent cx="2743200" cy="1371600"/><wp:docPr id="1" name="Synthetic object"/><a:graphic><a:graphicData uri="{uri}">{payload}</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>'
    chart = f'<c:chartSpace xmlns:c="{c}" xmlns:r="{R}"><c:chart><c:plotArea><c:barChart><c:ser><c:val><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:val></c:ser></c:barChart></c:plotArea></c:chart><c:externalData r:id="workbook"><c:autoUpdate val="0"/></c:externalData></c:chartSpace>'
    package(out / 'chart.docx', drawing(c, f'<c:chart xmlns:c="{c}" r:id="chart"/>'), [('chart', 'chart', 'charts/chart1.xml')], {
        'word/charts/chart1.xml': chart,
        'word/charts/_rels/chart1.xml.rels': relationships([('workbook', 'package', '../embeddings/workbook.xlsx')]),
        'word/embeddings/workbook.xlsx': b'SYNTHETIC-INERT-WORKBOOK-NOT-EXECUTABLE',
    }, {'word/charts/chart1.xml': 'application/vnd.openxmlformats-officedocument.drawingml.chart+xml', 'word/embeddings/workbook.xlsx': 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'})
    diagram_parts = {
        'word/diagrams/data1.xml': f'<dgm:dataModel xmlns:dgm="{d}" xmlns:a="{a}"><dgm:ptLst><dgm:pt modelId="0" type="doc"><dgm:t><a:p><a:r><a:t>Synthetic diagram node</a:t></a:r></a:p></dgm:t></dgm:pt></dgm:ptLst><dgm:cxnLst/></dgm:dataModel>',
        'word/diagrams/layout1.xml': f'<dgm:layoutDef xmlns:dgm="{d}" uniqueId="synthetic-layout"><dgm:layoutNode name="root"/></dgm:layoutDef>',
        'word/diagrams/style1.xml': f'<dgm:styleDef xmlns:dgm="{d}" uniqueId="synthetic-style"/>',
        'word/diagrams/colors1.xml': f'<dgm:colorsDef xmlns:dgm="{d}" uniqueId="synthetic-colors"/>',
    }
    entries = [('dm', 'diagramData', 'data1'), ('lo', 'diagramLayout', 'layout1'), ('qs', 'diagramQuickStyle', 'style1'), ('cs', 'diagramColors', 'colors1')]
    package(out / 'smartart.docx', drawing(d, f'<dgm:relIds xmlns:dgm="{d}" r:dm="dm" r:lo="lo" r:qs="qs" r:cs="cs"/>'), [(rid, typ, f'diagrams/{name}.xml') for rid, typ, name in entries], diagram_parts, {f'word/diagrams/{name}.xml': f'application/vnd.openxmlformats-officedocument.drawingml.{"diagramStyle" if typ == "diagramQuickStyle" else typ}+xml' for _, typ, name in entries})


def graph_invariant(z, doc, kind):
    # Resolve the referenced graph, allowing relocation but never missing parts,
    # changed payloads/content types, broken edges or external targets.
    ct = ET.fromstring(z.read('[Content_Types].xml'))
    overrides = {e.attrib['PartName'].lstrip('/'): e.attrib['ContentType'] for e in ct if 'PartName' in e.attrib}
    defaults = {e.attrib['Extension']: e.attrib['ContentType'] for e in ct if 'Extension' in e.attrib}
    def rels(part):
        name = posixpath.join(posixpath.dirname(part), '_rels', posixpath.basename(part) + '.rels')
        return list(ET.fromstring(z.read(name))) if name in z.namelist() else []
    def walk(part, ancestors=()):
        require(part not in ancestors and len(ancestors) < 16, 'Unexpected relationship cycle')
        data = z.read(part)
        content_type = overrides.get(part, defaults.get(part.rsplit('.', 1)[-1]))
        require(content_type is not None, 'Missing object content type')
        edges = []
        for rel in rels(part):
            require(rel.attrib.get('TargetMode') != 'External', 'Unexpected external relationship')
            target = posixpath.normpath(posixpath.join(posixpath.dirname(part), rel.attrib['Target']))
            edges.append((rel.attrib['Type'], walk(target, (*ancestors, part))))
        return (hashlib.sha256(data).hexdigest(), content_type, sorted(edges))
    roots = rels('word/document.xml')
    if kind == 'chart':
        nodes = list(doc.iter('{http://schemas.openxmlformats.org/drawingml/2006/chart}chart'))
        require(len(nodes) == 1, 'Chart reference lost')
        ids = [nodes[0].attrib[f'{{{R}}}id']]
    else:
        nodes = list(doc.iter('{http://schemas.openxmlformats.org/drawingml/2006/diagram}relIds'))
        require(len(nodes) == 1, 'SmartArt references lost')
        ids = [nodes[0].attrib[f'{{{R}}}{key}'] for key in ('dm', 'lo', 'qs', 'cs')]
    result = []
    for rid in ids:
        found = [r for r in roots if r.attrib['Id'] == rid]
        require(len(found) == 1 and found[0].attrib.get('TargetMode') != 'External', 'Object relationship lost')
        result.append((found[0].attrib['Type'], walk(posixpath.normpath('word/' + found[0].attrib['Target']))))
    return result


def invariant(path, kind):
    with zipfile.ZipFile(path) as z:
        doc = ET.fromstring(z.read('word/document.xml'))
        if kind in ('chart', 'smartart'):
            return graph_invariant(z, doc, kind)
        if kind == 'notes':
            result = {}
            for note in ('footnote', 'endnote'):
                refs = [n.attrib[f'{{{W}}}id'] for n in doc.iter(f'{{{W}}}{note}Reference')]
                require(len(refs) == 1, f'{note} reference lost')
                notes = ET.fromstring(z.read(f'word/{note}s.xml'))
                matches = [n for n in notes if n.attrib.get(f'{{{W}}}id') == refs[0]]
                require(len(matches) == 1, f'{note} target lost')
                result[note] = ''.join(n.text or '' for n in matches[0].iter(f'{{{W}}}t'))
            return result
        if kind == 'equation':
            equations = list(doc.iter(f'{{{M}}}oMath'))
            require(len(equations) == 1, 'Equation lost')
            return [(n.tag, n.text or '') for n in equations[0].iter() if n.tag in {f'{{{M}}}{x}' for x in ('f', 'num', 'den', 't')}]
        objects = list(doc.iter('{urn:schemas-microsoft-com:office:office}OLEObject'))
        require(len(objects) == 1, 'OLE reference lost')
        rid = objects[0].attrib[f'{{{R}}}id']
        rels = ET.fromstring(z.read('word/_rels/document.xml.rels'))
        rel = [r for r in rels if r.attrib.get('Id') == rid]
        require(len(rel) == 1 and rel[0].attrib['Type'] == R + '/oleObject', 'OLE relationship lost')
        target = posixpath.normpath('word/' + rel[0].attrib['Target'])
        image_rels = [r for r in rels if r.attrib.get('Type') == R + '/image']
        require(len(image_rels) == 1, 'OLE preview relationship lost')
        preview = z.read(posixpath.normpath('word/' + image_rels[0].attrib['Target']))
        require(preview == PNG, 'OLE preview bytes changed')
        payload = z.read(target)
        require(payload == OLE, 'Opaque OLE payload changed')
        return {'payload_sha256': hashlib.sha256(payload).hexdigest(), 'program': objects[0].attrib['ProgID']}


def run_cli(binary, *args):
    result = subprocess.run([str(binary), *map(str, args)], capture_output=True, timeout=60, check=False)
    require(result.returncode == 0, f'CLI failed: {args[0]}: {result.stderr.decode(errors="replace")[:500]}')
    return json.loads(result.stdout)


def verify(old, new, out):
    old, new, out = [Path(p).resolve() for p in (old, new, out)]
    require(not out.exists(), 'Output must be new')
    fingerprints = {'baseline': digest(old), 'candidate': digest(new)}
    out.mkdir(parents=True)
    generate(out)
    rows = []
    for kind in ('notes', 'equation', 'ole', 'chart', 'smartart'):
        source = out / f'{kind}.docx'
        original = digest(source)
        expected = invariant(source, kind)
        reports = {}
        for label, binary in [('baseline', old), ('candidate', new)]:
            reports[label] = run_cli(binary, 'inspect', '--compatibility', source)
        current = source
        for generation in range(2):
            saved = out / f'{kind}-candidate-{generation}.docx'
            run_cli(new, 'convert', current, saved)
            require(invariant(saved, kind) == expected, f'{kind}: roundtrip {generation} changed content')
            run_cli(new, 'inspect', '--compatibility', saved)
            run_cli(old, 'inspect', '--compatibility', saved)
            current = saved
        differing = [key for key in reports['baseline'].keys() | reports['candidate'].keys()
                     if reports['baseline'].get(key) != reports['candidate'].get(key)]
        require(set(differing) <= {'diagnostics'}, f'{kind}: semantic compatibility report changed')
        require(digest(source) == original, 'Source changed')
        rows.append({'case': kind, 'source_sha256': original, 'matching_reports': reports['baseline'] == reports['candidate'], 'reports': reports, 'differing_keys': differing, 'candidate_roundtrips': 2, 'baseline_readback': True, 'source_unchanged': True})
    require(digest(old) == fingerprints['baseline'] and digest(new) == fingerprints['candidate'], 'CLI binary changed')
    result = {'synthetic_only': True, 'binary_sha256': fingerprints, 'cases': rows, 'office_rendering_verified': False, 'ole_execution_verified': False}
    (out / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ('baseline', 'candidate', 'out'):
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.baseline, args.candidate, args.out)))
