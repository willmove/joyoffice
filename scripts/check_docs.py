"""Validate repository-local planning and implementation references."""
import csv
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]


def check():
    errors = []
    paths = [ROOT / 'README.md', ROOT / 'AGENTS.md', *ROOT.joinpath('docs').rglob('*.md')]
    for path in paths:
        text = path.read_text(encoding='utf-8')
        if text.count('```') % 2:
            errors.append(f'unclosed code fence: {path.relative_to(ROOT)}')
        for target in re.findall(r'\[[^\]]*\]\(([^)]+)\)', text):
            if re.match(r'\w+://', target) or target.startswith('#'):
                continue
            local = target.split('#')[0]
            if local and not (path.parent / local).exists():
                errors.append(f'broken link: {path.relative_to(ROOT)} -> {target}')
    tasks = (ROOT / 'docs/joydeck/TASKS.md').read_text(encoding='utf-8')
    ids = re.findall(r'^\| (?:待开始|进行中|已实现待验收|完成|阻塞) \| (JD-\d{3}) \|', tasks, re.M)
    if len(ids) != 98 or len(set(ids)) != 98:
        errors.append('the 98 implementation task IDs must remain unique')
    with (ROOT / 'docs/joydeck/COMMANDS.csv').open(encoding='utf-8-sig', newline='') as stream:
        commands = list(csv.DictReader(stream))
    if len(commands) != 329 or len({r['command_id'] for r in commands}) != 329:
        errors.append('command inventory IDs changed without a reviewed baseline update')
    for source in json.loads((ROOT / 'licenses/joyoffice/source-map.json').read_text(encoding='utf-8')):
        if not (ROOT / source['target_path']).is_file():
            errors.append('missing imported source: ' + source['target_path'])
    return errors


if __name__ == '__main__':
    errors = check()
    print('\n'.join(errors) if errors else 'Documentation and task references passed')
    sys.exit(bool(errors))
