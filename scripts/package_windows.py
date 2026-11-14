#!/usr/bin/env python3
"""Build a per-user NSIS installer from a verified product ZIP or staged payload."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

from verify_release_assets import digest_file, executable_header, filename, verify_archive

ROOT = Path(__file__).resolve().parents[1]
PRODUCTS = {
    'joyword': ('JoyWord', 'JoyWord.Document', 'Word document', ('.docx',), 'LICENSE'),
    'joydeck': ('JoyDeck', 'JoyDeck.Presentation', 'Presentation', ('.pptx', '.joydeck'), 'LICENSE'),
    # Keep the existing Sheet installation/registry identity for upgrades.
    'joysheet': ('Joysheet', 'Joysheet.Workbook', 'Workbook', ('.xlsx', '.csv', '.ods'), 'LICENSE-MIT'),
}
TARGETS = {'x86_64-pc-windows-msvc': 'x86_64', 'aarch64-pc-windows-msvc': 'aarch64'}


def nsis_literal(value):
    value = str(value)
    if any(ord(c) < 32 for c in value):
        raise ValueError('Control character in installer input')
    return value.replace('$', '$$').replace('"', '$\\"')


def compiler_path(explicit=None, product=None):
    candidates = [explicit, os.environ.get('JOYOFFICE_NSIS'),
                  os.environ.get((product or '').upper() + '_NSIS'), shutil.which('makensis.exe'),
                  ROOT / 'artifacts/joysheet/tools/nsis-3.11/makensis.exe',
                  Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'NSIS/makensis.exe']
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            compiler = Path(candidate).resolve()
            version = subprocess.check_output([str(compiler), '/VERSION'], text=True).strip()
            if version != 'v3.11':
                raise ValueError(f'NSIS 3.11 required, found {version}')
            if not (compiler.parent / 'COPYING').is_file():
                raise ValueError('NSIS COPYING notice is required')
            return compiler
    raise ValueError('Install NSIS 3.11 or set JOYOFFICE_NSIS to makensis.exe')


def payload_info(directory, product):
    info = json.loads((directory / 'BUILD-INFO.json').read_text(encoding='utf-8'))
    version = info.get('version', '')
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', version):
        raise ValueError('Invalid installer version')
    arch = TARGETS.get(info.get('target'))
    if arch is None or set(info.get('packageRoots', ())) != {product, product + '-cli'}:
        raise ValueError('Installer product/target metadata differs from payload')
    if not info.get('sourceSnapshotVerified'):
        raise ValueError('Installer requires a verified source snapshot')
    for name in (product, product + '-cli'):
        with (directory / (name + '.exe')).open('rb') as stream:
            executable_header(stream, 'windows', arch)
    return version, arch


def write_uninstall_inventory(payload, destination):
    files, directories = [], []
    for path in sorted(payload.rglob('*')):
        if path.is_symlink() or path.is_junction():
            raise ValueError('Installer payload may not contain links')
        relative = nsis_literal(path.relative_to(payload).as_posix().replace('/', '\\'))
        if path.is_file():
            files.append(f'Delete "$INSTDIR\\{relative}"')
        elif path.is_dir():
            directories.append((len(path.parts), f'RMDir "$INSTDIR\\{relative}"'))
        else:
            raise ValueError('Unsupported installer payload entry')
    # Never use recursive removal: unrelated files and application preferences survive.
    destination.write_text('\n'.join(files + [line for _, line in sorted(directories, reverse=True)]) + '\n',
                           encoding='utf-8')


def runtime_directory(arch):
    program_files = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)'))
    vswhere = program_files / 'Microsoft Visual Studio/Installer/vswhere.exe'
    component = 'Microsoft.VisualStudio.Component.VC.Tools.' + ('ARM64' if arch == 'aarch64' else 'x86.x64')
    installation = subprocess.check_output([str(vswhere), '-latest', '-products', '*', '-requires',
                                            component, '-property', 'installationPath'], text=True).strip()
    if not installation:
        raise ValueError('Matching Visual Studio C++ redistributable is required')
    root = Path(installation) / 'VC/Redist/MSVC'
    versions = sorted((p for p in root.iterdir() if p.is_dir() and re.fullmatch(r'v?\d+(?:\.\d+)*', p.name)),
                      key=lambda p: (p.name.startswith('v'), tuple(map(int, p.name.lstrip('v').split('.')))),
                      reverse=True)
    for version in versions:
        folder = version / ('arm64' if arch == 'aarch64' else 'x64')
        for candidate in sorted(folder.glob('Microsoft.VC*.CRT')):
            if (candidate / 'vcruntime140.dll').is_file():
                return candidate.resolve()
    raise ValueError('Matching Visual Studio C++ redistributable is missing')


def collect_runtime(payload, arch):
    # Preserve existing published runtime bytes (notably Sheet's app-local CRT).
    # For archives without a CRT, use only the licensed VS Redist release folder,
    # never System32, debug_nonredist or a DLL downloaded from an arbitrary site.
    origin = 'original payload'
    if not (payload / 'vcruntime140.dll').is_file():
        source = runtime_directory(arch)
        origin = '/'.join(source.parts[-4:])
        for path in source.glob('*.dll'):
            if not (payload / path.name).exists():
                shutil.copy2(path, payload / path.name)
    files = {p.name: digest_file(p) for p in sorted(payload.glob('*.dll'))
             if p.name.lower().startswith(('vcruntime', 'msvcp', 'concrt', 'vccorlib'))}
    if not any(name.lower() == 'vcruntime140.dll' for name in files):
        raise ValueError('App-local vcruntime140.dll is required')
    shutil.copy2(ROOT / 'licenses/joyoffice/MSVC-RUNTIME-NOTICE.txt', payload / 'licenses/MSVC-RUNTIME-NOTICE.txt')
    return {'origin': origin, 'arch': arch, 'sha256': files}


def compile_payload(directory, output, product, compiler, *, archive_sha256=None):
    version, arch = payload_info(directory, product)
    if output.exists() or output.with_name(output.name + '.sha256').exists():
        raise FileExistsError('Existing installer output is preserved: ' + str(output))
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=product + '-nsis-', dir=output.parent) as temporary:
        build = Path(temporary)
        payload = build / 'payload'
        # Reject links before copytree can dereference them.
        for path in directory.rglob('*'):
            if path.is_symlink() or path.is_junction():
                raise ValueError('Installer payload may not contain links')
        shutil.copytree(directory, payload)
        (payload / 'licenses').mkdir(exist_ok=True)
        shutil.copy2(compiler.parent / 'COPYING', payload / 'licenses/NSIS-COPYING.txt')
        runtime = collect_runtime(payload, arch)
        title, progid, description, extensions, license_file = PRODUCTS[product]
        (payload / license_file).read_bytes()
        provenance = {'product': product, 'version': version, 'target': info_target(arch),
                      'payloadArchiveSha256': archive_sha256, 'compiler': 'NSIS 3.11',
                      'publisherSigned': False, 'appLocalRuntime': runtime,
                      'installerSources': {name: digest_file(ROOT / name) for name in (
                          'scripts/package_windows.py', 'scripts/windows/joyoffice.nsi',
                          'scripts/verify_release_assets.py', 'licenses/joyoffice/MSVC-RUNTIME-NOTICE.txt')}}
        (payload / 'INSTALLER-INFO.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
        write_uninstall_inventory(payload, build / 'uninstall-files.nsh')
        definitions = {'PRODUCT': product, 'TITLE': title, 'DISPLAY_NAME': f'{title} {version} preview',
                       'PROGID': progid, 'DESCRIPTION': description, 'VERSION': version,
                       'NUMERIC_VERSION': version.split('-')[0] + '.0', 'PAYLOAD_ARCH': arch,
                       'LICENSE_FILE': license_file, 'PAYLOAD': payload,
                       'OUTPUT': output.resolve(), 'ICON': ROOT / f'assets/brand/{product}.ico'}
        (build / 'config.nsh').write_text(''.join(
            f'!define {key} "{nsis_literal(value)}"\n' for key, value in definitions.items()), encoding='utf-8')
        (build / 'associations.nsh').write_text(''.join(
            f'WriteRegStr HKCU "Software\\Classes\\{ext}\\OpenWithProgids" "${{PROGID}}" ""\n'
            for ext in extensions), encoding='utf-8')
        (build / 'uninstall-associations.nsh').write_text(''.join(
            f'DeleteRegValue HKCU "Software\\Classes\\{ext}\\OpenWithProgids" "${{PROGID}}"\n'
            for ext in extensions), encoding='utf-8')
        shutil.copy2(ROOT / 'scripts/windows/joyoffice.nsi', build / 'joyoffice.nsi')
        subprocess.run([str(compiler), '/V2', str(build / 'joyoffice.nsi')], cwd=build, check=True)
    with output.open('rb') as stream:
        executable_header(stream, 'windows', 'x86')
    output.with_name(output.name + '.sha256').write_text(
        f'{digest_file(output)}  {output.name}\n', encoding='ascii')
    print(output)


def info_target(arch):
    return next(target for target, value in TARGETS.items() if value == arch)


def package_archive(archive, product, version, arch, output, compiler):
    if archive.name != filename(product, version, 'windows', arch):
        raise ValueError('Archive filename differs from selected product/version/architecture')
    checksum = digest_file(archive)
    if archive.with_name(archive.name + '.sha256').read_text(encoding='ascii').split() != [checksum, archive.name]:
        raise ValueError('Archive SHA-256 sidecar differs')
    with zipfile.ZipFile(archive) as package:
        if len(package.infolist()) > 20_000 or sum(item.file_size for item in package.infolist()) > 2 * 1024**3:
            raise ValueError('Installer archive exceeds extraction budget')
        verify_archive(archive, product, version, 'windows', arch)
        with tempfile.TemporaryDirectory(prefix=product + '-archive-') as temporary:
            # verify_archive validates every path, duplicate, type and platform name first.
            package.extractall(temporary)
            payload = Path(temporary) / (archive.stem if product != 'joyword' else '')
            if payload_info(payload, product) != (version, arch):
                raise ValueError('Installer archive metadata differs')
            compile_payload(payload, output, product, compiler, archive_sha256=checksum)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--product', choices=PRODUCTS, required=True)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument('--archive', type=Path)
    source.add_argument('--directory', type=Path)
    parser.add_argument('--version')
    parser.add_argument('--arch', choices=['x86_64', 'aarch64'])
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--compiler', type=Path)
    args = parser.parse_args()
    compiler = compiler_path(args.compiler, args.product)
    if args.archive:
        if not args.version or not args.arch:
            parser.error('--archive requires --version and --arch')
        package_archive(args.archive, args.product, args.version, args.arch, args.output, compiler)
    else:
        compile_payload(args.directory, args.output, args.product, compiler)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        sys.exit(str(error))
