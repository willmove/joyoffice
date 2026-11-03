"""Check the exact JoyOffice dependency baseline and headless isolation."""
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[2]
FORBIDDEN = re.compile(r'^(egui(?:[-_].*)?|eframe|epaint(?:[-_].*)?|craft-ui)$')
EXPECTED = {'krilla': '0.8.2', 'gpui-kit': '0.8.1', 'gpui-base': '0.8.1', 'gpui-component': '0.8.1', 'gpui-kit-assets': '0.8.1', 'gpui-pre': '0.3.10', 'gpui-pre-platform': '0.3.10', 'serde': '1.0.228', 'serde_json': '1.0.154'}


def check_lock(lock):
    errors = []
    packages = lock['package']
    for name, version in EXPECTED.items():
        matches = [p for p in packages if p['name'] == name]
        if len(matches) != 1 or matches[0]['version'] != version:
            errors.append(f'{name} must resolve exactly once to {version}')
        elif matches[0].get('source') != 'registry+https://github.com/rust-lang/crates.io-index':
            errors.append(f'{name} has an unexpected dependency source')
    for package in packages:
        if FORBIDDEN.fullmatch(package['name']):
            errors.append('egui dependency entered the workspace: ' + package['name'])
    return errors


def main():
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text(encoding='utf-8'))
    errors = check_lock(lock)
    for package in ['joydeck-cli', 'joydeck-shell', 'joydeck-engine', 'joydeck-color', 'joydeck-geom', 'joydeck-model', 'joydeck-render', 'joydeck-pptx', 'joydeck-format', 'joydeck-pdf', 'joydeck-legacy']:
        tree = subprocess.check_output(['cargo', 'tree', '--locked', '--offline', '-p', package, '--edges', 'normal,build', '--prefix', 'none'], cwd=ROOT, encoding='utf-8')
        for line in tree.splitlines():
            name = line.split(' ', 1)[0]
            if 'gpui' in name or name.startswith(('joyword', 'joysheet')):
                errors.append(f'{package} acquired a GUI/other-product dependency: ' + name)
    features = subprocess.check_output(['cargo', 'tree', '--locked', '--offline', '-p', 'joydeck', '-e', 'features'], cwd=ROOT, encoding='utf-8')
    if 'gpui-fast' in features:
        errors.append('gpui-fast was enabled without a baseline change')
    workspace = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))
    if workspace['workspace']['dependencies']['krilla'] != {'version': '=0.8.2', 'default-features': False} or tomllib.loads((ROOT / 'crates/joydeck-pdf/Cargo.toml').read_text(encoding='utf-8'))['dependencies']['krilla'] != {'workspace': True, 'features': ['raster-images']}:
        errors.append('Krilla must match JoyOffice official 0.8.2 raster-images-only baseline')
    for name, pin in {'sha2': '=0.10.9', 'getrandom': '=0.3.4'}.items():
        if workspace['workspace']['dependencies'][name] != pin:
            errors.append(f'{name} must match the JoyOffice direct pin {pin}')
    for path in (ROOT / 'crates/joydeck-engine/src').rglob('*.rs'):
        if re.search(r'std::(?:fs|io|process)|joydeck_(?:pdf|media|shell)', path.read_text(encoding='utf-8')):
            errors.append('IO/service dependency in pure engine: ' + str(path.relative_to(ROOT)))
    print('\n'.join(errors) if errors else 'Exact dependency baseline and CLI/core isolation passed')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())
