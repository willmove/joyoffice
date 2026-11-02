"""Product-scoped JoyDeck engineering commands (Python 3.11+)."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def native_environment():
    env = os.environ.copy()
    env['PYTHONUTF8'] = '1'
    if sys.platform == 'win32' and not any((Path(p) / 'cmake.exe').is_file() for p in env.get('PATH', '').split(os.pathsep)):
        vswhere = Path(env.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
        if vswhere.is_file():
            install = subprocess.check_output([str(vswhere), '-latest', '-products', '*', '-property', 'installationPath'], text=True, encoding='utf-8').strip()
            cmake = Path(install) / 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin'
            if (cmake / 'cmake.exe').is_file():
                env['PATH'] = str(cmake) + os.pathsep + env.get('PATH', '')
    return env


def run(args):
    print('+ ' + ' '.join(str(a) for a in args), flush=True)
    subprocess.run([str(a) for a in args], cwd=ROOT, env=native_environment(), check=True)


def selected_packages(headless):
    data = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--offline', '--no-deps', '--format-version', '1'], cwd=ROOT, encoding='utf-8'))
    packages = {p['name']: p for p in data['packages']}

    def requires_gui(name, seen):
        if name in seen:
            return False
        if name.startswith(('gpui', 'egui', 'eframe')) or name in {'joyoffice-ui', 'joydeck-ui-gpui', 'joydeck'}:
            return True
        package = packages.get(name)
        if package is None:
            return False
        return any(requires_gui(d.get('rename') or d['name'], seen | {name}) for d in package['dependencies'] if d['kind'] != 'dev')

    return sorted(name for name in packages if not headless or not requires_gui(name, set()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['build', 'check', 'test', 'run', 'package'])
    parser.add_argument('product', choices=['joydeck'], nargs='?', default='joydeck')
    parser.add_argument('--headless', action='store_true')
    parser.add_argument('--release', action='store_true')
    parser.add_argument('--validation', action='store_true')
    parser.add_argument('--target')
    args = parser.parse_args()
    if args.headless and args.validation:
        parser.error('--validation applies to desktop builds')
    if args.operation == 'package':
        if args.headless:
            parser.error('package includes the desktop application')
        run([sys.executable, ROOT / 'scripts/joydeck/package-native.py', '--profile', 'release' if args.release else 'debug'])
        return
    if args.operation == 'run':
        if not args.headless and sys.platform == 'win32':
            run([sys.executable, ROOT / 'scripts/joydeck/build-windows-helpers.py', '--output', ROOT / ('target/release' if args.release else 'target/debug')])
        run(['cargo', 'run', '--locked', '-p', 'joydeck-cli' if args.headless else 'joydeck', *(['--release'] if args.release else [])])
        return
    packages = selected_packages(args.headless)
    flags = [arg for package in packages for arg in ['-p', package]]
    if args.release:
        flags += ['--release']
    if args.validation:
        flags += ['--features', 'joydeck/validation']
    if args.target:
        flags += ['--target', args.target]
    if args.operation == 'check':
        run([sys.executable, ROOT / 'scripts/check_docs.py'])
        run([sys.executable, ROOT / 'scripts/check_dependencies.py'])
        run([sys.executable, ROOT / 'scripts/check_sources.py'])
        run([sys.executable, '-m', 'unittest', 'discover', '-s', 'scripts/tests', '-p', 'test_*.py'])
        run(['cargo', 'fmt', '--all', '--', '--check'])
        run(['cargo', 'clippy', '--locked', '--all-targets', *flags, '--', '-D', 'warnings'])
        run(['cargo', 'test', '--locked', *flags])
    else:
        run(['cargo', args.operation, '--locked', *flags])
        if args.operation == 'build' and not args.headless and sys.platform == 'win32':
            run([sys.executable, ROOT / 'scripts/joydeck/build-windows-helpers.py', '--output', ROOT / ('target/release' if args.release else 'target/debug')])


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'JoyDeck engineering command failed: {error}', file=sys.stderr)
        sys.exit(1)
