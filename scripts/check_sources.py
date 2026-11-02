"""Verify imported source files against their recorded adaptation hashes."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    records = json.loads((ROOT / 'licenses/joyoffice/source-map.json').read_text(encoding='utf-8'))
    deckcraft = ROOT / 'licenses/deckcraft/file-map.json'
    if deckcraft.is_file():
        records += json.loads(deckcraft.read_text(encoding='utf-8'))['files']
    fonts = json.loads((ROOT / 'licenses/fonts/sources.json').read_text(encoding='utf-8'))
    records += [{'target_path': r['path'], 'source_sha256': r['sha256']} for r in fonts['files']]
    errors = []
    for record in records:
        path = (ROOT / record['target_path']).resolve()
        if not path.is_relative_to(ROOT) or not path.is_file():
            errors.append('invalid imported target: ' + record['target_path'])
            continue
        expected = record.get('adapted_sha256', record['source_sha256'])
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            errors.append('unrecorded imported-source modification: ' + record['target_path'])
    print('\n'.join(errors) if errors else f'Imported source records passed ({len(records)} files)')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())
