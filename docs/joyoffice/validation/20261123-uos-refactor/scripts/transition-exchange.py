"""Check 48 remaining-transition file options through two CLI writers and readers.

Run from the JoyOffice repository root. This checks PPTX attributes, not rendered
PowerPoint parity; hardware composition and real slideshow evidence are separate.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED

P = 'http://schemas.openxmlformats.org/presentationml/2006/main'
P14 = 'http://schemas.microsoft.com/office/powerpoint/2010/main'
OPTIONS = {
    'reveal': ['smoothlyLeft', 'smoothlyRight', 'blackLeft', 'blackRight'],
    'flash': [''], 'honeycomb': [''],
    'glitter': ['l', 'r', 'u', 'd', 'hexL', 'hexR', 'hexU', 'hexD'],
    'vortex': ['l', 'r', 'u', 'd'],
    'shred': ['stripsIn', 'stripsOut', 'particlesIn', 'particlesOut'],
    'switch': ['l', 'r'], 'flip': ['l', 'r'], 'gallery': ['l', 'r'],
    'doors': ['vert', 'horz'], 'zoom': ['in', 'out', 'rotate'],
    'pan': ['u', 'd', 'l', 'r'], 'ferris': ['l', 'r'], 'conveyor': ['l', 'r'],
    'window': ['vert', 'horz'], 'flythrough': ['in', 'out', 'inBounce', 'outBounce'],
    'random': [''],
}


def element(kind, option):
    tag, namespace, attrs = kind, P14, {}
    if kind == 'random':
        namespace = P
    elif kind == 'reveal':
        attrs = {'dir': 'r' if option.endswith('Right') else 'l', 'thruBlk': str(int(option.startswith('black')))}
    elif kind == 'glitter':
        attrs = {'dir': option.removeprefix('hex').lower()}
        if option.startswith('hex'):
            attrs['pattern'] = 'hexagon'
    elif kind == 'shred':
        attrs = {'pattern': 'rectangle' if option.startswith('particles') else 'strip', 'dir': 'out' if option.endswith('Out') else 'in'}
    elif kind == 'zoom':
        tag = 'newsflash' if option == 'rotate' else 'warp'
        if option == 'rotate':
            namespace = P
        else:
            attrs = {'dir': option}
    elif kind == 'flythrough':
        attrs = {'dir': 'out' if option.startswith('out') else 'in', 'hasBounce': str(int(option.endswith('Bounce')))}
    elif option:
        attrs = {'dir': option}
    return f'{{{namespace}}}{tag}', attrs


def verify(path, expected):
    with ZipFile(path) as z:
        root = ET.fromstring(z.read('ppt/slides/slide2.xml'))
    transition = root.find('.//{*}transition')
    assert transition is not None and transition.get(f'{{{P14}}}dur') == '4800', path
    tag, attrs = expected
    child = transition.find(tag)
    assert child is not None, (path, tag)
    for key, value in attrs.items():
        actual = child.get(key, '0')
        assert (actual in ('true', '1')) == (value == '1') if key in ('thruBlk', 'hasBounce') else actual == value, (path, key, actual, value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ('baseline', 'candidate', 'output'):
        parser.add_argument(arg, type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    binaries = {key: getattr(args, key).resolve() for key in ('baseline', 'candidate')}
    env = {**os.environ, 'XDG_CONFIG_HOME': str(out / 'config'), 'XDG_STATE_HOME': str(out / 'state')}
    fixture = Path('tests/fixtures/joydeck/independent.pptx')
    results = []
    for kind, options in OPTIONS.items():
        for option in options:
            folder = out / (kind + '-' + (option or 'default'))
            folder.mkdir()
            source = folder / 'source.pptx'
            expected = element(kind, option)
            with ZipFile(fixture) as src, ZipFile(source, 'x', ZIP_DEFLATED) as dst:
                for name in src.namelist():
                    data = src.read(name)
                    if name == 'ppt/slides/slide2.xml':
                        root = ET.fromstring(data)
                        for old in root.findall('{*}transition'):
                            root.remove(old)
                        tr = ET.SubElement(root, f'{{{P}}}transition', {f'{{{P14}}}dur': '4800'})
                        ET.SubElement(tr, expected[0], expected[1])
                        data = ET.tostring(root, encoding='utf-8', xml_declaration=True)
                    dst.writestr(name, data)
            source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
            for writer_name, writer in binaries.items():
                saved = folder / (writer_name + '.pptx')
                subprocess.run([writer, 'exchange', source, '--mode', 'regenerate', '--output', saved], env=env, check=True, capture_output=True, timeout=30)
                verify(saved, expected)
                for reader_name, reader in binaries.items():
                    reopened = folder / (writer_name + '-' + reader_name + '.pptx')
                    subprocess.run([reader, 'exchange', saved, '--mode', 'regenerate', '--output', reopened], env=env, check=True, capture_output=True, timeout=30)
                    verify(reopened, expected)
            assert hashlib.sha256(source.read_bytes()).hexdigest() == source_hash
            results.append({'kind': kind, 'option': option, 'sourceSha256': source_hash, 'readerWriterPairs': 4})
    assert len(OPTIONS) == 17 and len(results) == 48
    report = {'kinds': 17, 'cases': 48, 'readerWriterPairs': 192, 'binaries': {k: hashlib.sha256(p.read_bytes()).hexdigest() for k, p in binaries.items()}, 'originalsUnchanged': True, 'casesVerified': results}
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
