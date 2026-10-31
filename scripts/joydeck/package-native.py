"""Build and package JoyDeck independently from the monorepo workspace."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import zipfile
from paths import ROOT, SCRIPTS, FIXTURES, LICENSES, PACKAGING, WORKSPACE
import sbom


def product_version(application='joydeck'):
    text = (ROOT / 'apps' / application / 'Cargo.toml').read_text()
    section = re.search(r'(?ms)^\[package\]\s*(.*?)(?=^\[|\Z)', text)
    match = re.search(r'^version\s*=\s*"([^"]+)"', section[1] if section else '', re.M)
    if not match:
        raise ValueError('JoyDeck must declare its independent application version')
    return match[1]


def build_binary(package, profile, validation=False):
    rustc = subprocess.check_output(['rustc', '-vV'], cwd=ROOT, text=True)
    host = next(line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('host:'))
    command = ['cargo', 'build', '--locked', '-p', package, '--target', host, '--message-format=json-render-diagnostics']
    if profile == 'release':
        command.append('--release')
    if validation and package == 'joydeck':
        command += ['--features', 'validation']
    completed = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True)
    artifacts = []
    for line in completed.stdout.splitlines():
        try:
            value = json.loads(line)
        except ValueError:
            print(line)
            continue
        if value.get('reason') == 'compiler-artifact' and value.get('target', {}).get('name') == package and value.get('executable'):
            artifacts.append(Path(value['executable']))
    if len(artifacts) != 1:
        raise ValueError('Cargo did not report exactly one executable for ' + package)
    return artifacts[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name', help='New package directory name; existing outputs are preserved')
    parser.add_argument('--stage', choices=['m' + str(n) for n in range(4, 15)], default='m14')
    parser.add_argument('--profile', choices=['release', 'debug'], default='release')
    parser.add_argument('--validation', action='store_true', help='Include explicit native acceptance commands; not a production release')
    parser.add_argument('--out', type=Path, default=ROOT / 'dist/joydeck')
    args = parser.parse_args()
    if not WORKSPACE:
        raise ValueError('Packaging requires the source workspace, not an installed package')
    if args.name and not re.fullmatch(r'joydeck-[a-zA-Z0-9.+-]+', args.name):
        raise ValueError('Package name must start with joydeck- and contain only version/name characters')
    version = product_version()
    if product_version('joydeck-cli') != version:
        raise ValueError('JoyDeck CLI and application versions must match')
    system = {'Darwin': 'macos', 'Windows': 'windows', 'Linux': 'linux'}[platform.system()]
    arch = {'arm64': 'aarch64', 'aarch64': 'aarch64', 'AMD64': 'x86_64', 'x86_64': 'x86_64'}[platform.machine()]
    name = args.name or f'joydeck-{version}-{system}-{arch}-{args.profile}'
    output = args.out.resolve() / name
    archive = output.with_name(output.name + '.zip')
    if output.exists() or archive.exists():
        raise FileExistsError('Choose a new package name or output directory: ' + str(output))
    app_binary = build_binary('joydeck', args.profile, args.validation)
    cli_binary = build_binary('joydeck-cli', args.profile)
    output.mkdir(parents=True)
    binary, cli = app_binary.name, cli_binary.name
    shutil.copy2(app_binary, output / binary)
    shutil.copy2(cli_binary, output / cli)
    helpers = []
    if system == 'macos':
        for helper in ['media', 'print', 'video']:
            executable = 'joydeck-' + helper + '-macos'
            source = ROOT / 'crates/joydeck-platform/native' / (helper + '-macos.swift')
            subprocess.run(['swiftc', '-O', str(source), '-o', str(output / executable)], check=True)
            helpers.append(executable)
    for filename in ['LICENSE-MIT', 'LICENSE-APACHE', 'THIRD_PARTY.md']:
        shutil.copy2(LICENSES / filename, output / filename)
    for filename in ['Cargo.lock', 'rust-toolchain.toml']:
        shutil.copy2(ROOT / filename, output / filename)
    shutil.copy2(PACKAGING / ('README-' + args.stage + '.md'), output / 'README.md')
    shutil.copytree(LICENSES, output / 'licenses')
    shutil.copytree(FIXTURES, output / 'fixtures')
    shutil.copytree(SCRIPTS, output / 'scripts', ignore=shutil.ignore_patterns('__pycache__', '*.pyc', 'tests'))
    if system == 'macos':
        contents = output / 'JoyDeck.app/Contents'
        (contents / 'MacOS').mkdir(parents=True)
        plist = {'CFBundleExecutable': 'JoyDeck', 'CFBundleIdentifier': 'dev.joyoffice.joydeck',
                 'CFBundleName': 'JoyDeck', 'CFBundlePackageType': 'APPL', 'CFBundleVersion': version,
                 'CFBundleShortVersionString': version, 'NSHighResolutionCapable': True,
                 'CFBundleDocumentTypes': [
                     {'CFBundleTypeName': 'JoyDeck Document', 'CFBundleTypeRole': 'Editor',
                      'LSHandlerRank': 'Owner', 'CFBundleTypeExtensions': ['joydeck'],
                      'LSItemContentTypes': ['dev.joydeck.document']},
                     {'CFBundleTypeName': 'Office Presentation Import', 'CFBundleTypeRole': 'Viewer',
                      'LSHandlerRank': 'Alternate', 'CFBundleTypeExtensions': ['pptx', 'potx', 'ppsx']}],
                 'UTExportedTypeDeclarations': [
                     {'UTTypeIdentifier': 'dev.joydeck.document', 'UTTypeDescription': 'JoyDeck Document',
                      'UTTypeConformsTo': ['public.zip-archive'],
                      'UTTypeTagSpecification': {'public.filename-extension': ['joydeck']}}]}
        (contents / 'Info.plist').write_bytes(plistlib.dumps(plist))
        shutil.copy2(output / binary, contents / 'MacOS/JoyDeck')
        for executable in [cli, *helpers]:
            shutil.copy2(output / executable, contents / 'MacOS' / executable)
        resources = contents / 'Resources'
        resources.mkdir()
        for filename in ['README.md', 'THIRD_PARTY.md', 'LICENSE-MIT', 'LICENSE-APACHE']:
            shutil.copy2(output / filename, resources / filename)
        shutil.copytree(output / 'licenses', resources / 'licenses')
        launch = output / 'Launch.command'
        launch.write_text('#!/bin/sh\ncd -- "$(dirname -- "$0")" || exit 1\nexec ./joydeck\n')
        launch.chmod(0o755)
    rustc = subprocess.check_output(['rustc', '-vV'], cwd=ROOT, text=True)
    target = next(line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('host:'))
    features = ('joydeck/validation',) if args.validation else ()
    dependency_info = sbom.generate(output / 'SBOM.spdx.json', target, args.profile, features=features)
    source_hashes = dependency_info.pop('sourceHashes')
    (output / 'SOURCE-HASHES.json').write_text(json.dumps(source_hashes, indent=2) + '\n')
    info = {'version': version, 'stage': args.stage, 'target': target, 'profile': args.profile,
            'features': ['desktop', *(['validation'] if args.validation else [])],
            'packageRoots': ['joydeck', 'joydeck-cli'], 'nativeReaderMax': 11, 'writerSchema': '1.10',
            'rustc': rustc, 'cargoLockSha256': dependency_info['lockSha256'],
            'sourceGraphSha256': hashlib.sha256(json.dumps(source_hashes, sort_keys=True).encode()).hexdigest(),
            'dependencyPackages': dependency_info['packages'], 'officialSignatureVerified': False}
    (output / 'BUILD-INFO.json').write_text(json.dumps(info, indent=2) + '\n')
    if system == 'macos':
        resources = output / 'JoyDeck.app/Contents/Resources'
        for filename in ['BUILD-INFO.json', 'SBOM.spdx.json', 'SOURCE-HASHES.json']:
            shutil.copy2(output / filename, resources / filename)
        shutil.copytree(output / 'dependency-licenses', resources / 'dependency-licenses')
    hashes = {p.relative_to(output).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in sorted(output.rglob('*')) if p.is_file()}
    (output / 'SHA256.json').write_text(json.dumps(hashes, indent=2) + '\n')
    with zipfile.ZipFile(archive, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=6, strict_timestamps=False) as zipped:
        for path in sorted(output.rglob('*')):
            if path.is_file():
                zipped.write(path, path.relative_to(output.parent))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + '.sha256').write_text(digest + '  ' + archive.name + '\n')
    print(archive)

if __name__ == '__main__':
    main()
