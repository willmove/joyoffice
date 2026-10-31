"""Locate JoyDeck assets in the monorepo or a self-contained development package."""
from functools import lru_cache
import json
import os
from pathlib import Path
import subprocess

SCRIPTS = Path(__file__).resolve().parent
_candidate = SCRIPTS.parents[1]
WORKSPACE = (_candidate / 'Cargo.toml').is_file() and (_candidate / 'apps/joydeck').is_dir()
ROOT = _candidate if WORKSPACE else SCRIPTS.parent
FIXTURES = ROOT / ('tests/fixtures/joydeck' if WORKSPACE else 'fixtures')
EVIDENCE = ROOT / ('target/joydeck-verification' if WORKSPACE else 'verification')
LICENSES = ROOT / ('licenses/joydeck' if WORKSPACE else 'licenses')
PACKAGING = ROOT / ('packaging/joydeck' if WORKSPACE else '.')

@lru_cache(maxsize=1)
def target_directory():
    if not WORKSPACE:
        return ROOT
    if os.environ.get('CARGO_TARGET_DIR'):
        value = Path(os.environ['CARGO_TARGET_DIR'])
        return value if value.is_absolute() else ROOT / value
    data = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--no-deps', '--locked', '--format-version', '1'],
        cwd=ROOT, text=True))
    return Path(data['target_directory'])

def binary(name='joydeck', profile='debug'):
    filename = name + ('.exe' if os.name == 'nt' else '')
    return target_directory() / profile / filename if WORKSPACE else ROOT / filename

def native_helper(name='media', profile='debug'):
    executable = 'joydeck-' + name + '-macos'
    if not WORKSPACE:
        return ROOT / executable
    arguments = ['cargo', 'build', '-p', 'joydeck-platform', '--locked',
                 '--message-format=json-render-diagnostics']
    if profile == 'release':
        arguments.append('--release')
    output = subprocess.check_output(arguments, cwd=ROOT, text=True)
    for line in output.splitlines():
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if value.get('reason') == 'build-script-executed' and 'joydeck-platform' in value.get('package_id', ''):
            candidate = Path(value['out_dir']) / executable
            if candidate.is_file():
                return candidate
    raise FileNotFoundError('Cargo did not build the requested macOS helper: ' + executable)
