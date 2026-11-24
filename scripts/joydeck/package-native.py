"""Build and package the current JoyDeck source; never re-label an older binary."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/xtask'))
sys.path.insert(0, str(ROOT / 'scripts'))
from product import manifest_version, native_environment  # noqa: E402
from package_installers import build_linux_deb, build_macos_dmg, create_tar_gz_from_directory, write_sha256_sidecar  # noqa: E402
from product_icons import BRAND, bundle_icon, copy_logo, copy_linux_installer, linux_launcher  # noqa: E402


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_snapshot():
    paths = [ROOT / p for p in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'rustfmt.toml', '.gitattributes', 'LICENSE', 'THIRD_PARTY-JOYDECK.md']]
    paths += [BRAND / f'{product}.{ext}' for product in ('joyword', 'joydeck', 'joysheet') for ext in ('svg', 'ico')]
    paths += [BRAND / 'joydeck.icns']
    for folder in ['.cargo', 'apps', 'crates', 'tools', 'scripts', 'tests/fixtures', 'licenses', 'packaging', 'third-party']:
        paths += [p for p in (ROOT / folder).rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix != '.pyc']
    records = {p.relative_to(ROOT).as_posix(): sha256(p) for p in sorted(set(paths))}
    combined = hashlib.sha256(json.dumps(records, sort_keys=True).encode()).hexdigest()
    return {'sha256': combined, 'files': records}


def output_name(version, system, arch, profile, fingerprint):
    return f'joydeck-{version}-r9-{system}-{arch}-{profile}-{fingerprint[:10]}'


def create_archive(output, archive):
    """Publish a complete ZIP, including dependency files with epoch timestamps."""
    output, archive = output.resolve(), archive.resolve()
    temporary = archive.with_name(archive.name + '.partial')
    if archive.parent != output.parent or temporary.parent != output.parent:
        raise ValueError('Archive must remain beside its package directory')
    if archive.exists() or temporary.exists():
        raise FileExistsError('Existing archive is preserved: ' + str(archive))
    with zipfile.ZipFile(temporary, 'x', compression=zipfile.ZIP_DEFLATED, strict_timestamps=False) as bundle:
        for path in sorted(output.rglob('*')):
            if path.is_file():
                bundle.write(path, output.name + '/' + path.relative_to(output).as_posix())
    temporary.rename(archive)


def build_binary(package, profile, validation=False):
    rustc = subprocess.check_output(["rustc", "-vV"], cwd=ROOT, encoding="utf-8")
    host = next(line.split(": ", 1)[1] for line in rustc.splitlines() if line.startswith("host:"))
    command = ['cargo', 'build', '--locked', '-p', package, '--target', host, '--message-format=json-render-diagnostics']
    if profile == 'release':
        command.append('--release')
    if validation and package == 'joydeck':
        command += ['--features', 'validation']
    result = subprocess.run(command, cwd=ROOT, env=native_environment(), check=True, stdout=subprocess.PIPE, encoding='utf-8')
    artifacts = []
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == package and message.get('executable'):
            artifacts.append(Path(message['executable']))
    if len(artifacts) != 1:
        raise ValueError(f'Cargo did not report one executable for {package}')
    return artifacts[0]


def sbom(target):
    # Include host build-dependency identities; the target tree selects membership.
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--offline', '--format-version', '1'], cwd=ROOT, encoding='utf-8'))
    tree = subprocess.check_output(['cargo', 'tree', '--locked', '--offline', '--target', target, '-p', 'joydeck', '-p', 'joydeck-cli', '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}'], cwd=ROOT, encoding='utf-8')
    selected = set()
    for line in tree.splitlines():
        match = re.match(r'^(\S+) v([^\s]+)', line)
        if match:
            selected.add(match.groups())
    packages = [p for p in metadata['packages'] if (p['name'], p['version']) in selected]
    if len(packages) != len(selected):
        raise ValueError('Selected SBOM package identity is ambiguous or missing')
    return packages


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', choices=['debug', 'release'], default='release')
    parser.add_argument('--name')
    parser.add_argument('--validation', action='store_true')
    parser.add_argument('--evidence', type=Path, default=ROOT / 'docs/joydeck/evidence/R9/package.json',
                        help='Write the build report to a separate path for qualification runs')
    parser.add_argument('--release-layout', action='store_true', help='Use the suite release archive contract; native development packages retain R9 source fingerprints.')
    args = parser.parse_args()
    if args.name and not re.fullmatch(r'joydeck-[a-zA-Z0-9.+-]+', args.name):
        parser.error('--name must be a simple joydeck- package name')
    version = manifest_version('joydeck')
    cli_version = manifest_version('joydeck-cli')
    if version != cli_version:
        raise ValueError('Application and CLI versions must match')
    subprocess.run([sys.executable, ROOT / 'scripts/joydeck/check_sources.py'], cwd=ROOT, env=native_environment(), check=True)
    before = source_snapshot()
    binaries = [build_binary(package, args.profile, args.validation) for package in ['joydeck', 'joydeck-cli']]
    rustc = subprocess.check_output(['rustc', '-vV'], cwd=ROOT, encoding='utf-8')
    target = next(line.split(': ', 1)[1] for line in rustc.splitlines() if line.startswith('host:'))
    dependencies = sbom(target)
    if before != source_snapshot():
        raise ValueError('Source changed during packaging; rebuild the new source')
    system = {'Windows': 'windows', 'Darwin': 'macos', 'Linux': 'linux'}[platform.system()]
    arch = target.split('-', 1)[0]
    if args.release_layout and args.profile != 'release':
        raise ValueError('The suite release layout requires the release profile')
    name = args.name or (f'joydeck-{version}-{system}-{arch}-release' if args.release_layout else None) or output_name(version, system, arch, args.profile, before['sha256'])
    output = ROOT / 'dist/joydeck' / name
    archive = output.with_name(output.name + '.zip')
    tar_archive = output.with_name(output.name + '.tar.gz')
    deb_package = output.with_name(output.name + '.deb')
    dmg_package = output.with_name(output.name + '.dmg')
    extra_outputs = [tar_archive, deb_package] if system == 'linux' else ([dmg_package] if system == 'macos' else [])
    if any(path.exists() for path in (output, archive, *extra_outputs)):
        raise FileExistsError(f'Existing output is preserved: {output}')
    output.mkdir(parents=True)
    if system == 'windows':
        import importlib.util
        spec = importlib.util.spec_from_file_location('windows_helpers', ROOT / 'scripts/joydeck/build-windows-helpers.py')
        helpers = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(helpers)
        helpers.build(output)
    elif system == 'macos':
        subprocess.run([sys.executable, ROOT / 'scripts/joydeck/build-macos-helpers.py',
                        '--output', output, '--target', target], check=True)
    for binary in binaries:
        shutil.copy2(binary, output / binary.name)
    for file in ['LICENSE', 'THIRD_PARTY-JOYDECK.md', 'Cargo.lock', 'rust-toolchain.toml']:
        shutil.copy2(ROOT / file, output / file)
    shutil.copytree(ROOT / 'licenses', output / 'licenses')
    readme = (ROOT / 'packaging/joydeck/README-r9.md').read_text(encoding='utf-8')
    label = '软件包' if args.release_layout else '开发包'
    readme = readme.replace('@VERSION@', version).replace('@PACKAGE_LABEL@', label)
    (output / 'README.md').write_text(readme, encoding='utf-8')
    shutil.copytree(ROOT / 'tests/fixtures/joydeck', output / 'samples')
    copy_logo(output, 'joydeck')
    license_records = []
    for package in dependencies:
        dest = output / 'licenses/dependencies' / (package['name'] + '-' + package['version'])
        base = Path(package['manifest_path']).parent
        candidates = {p for pattern in ['LICENSE*', 'COPYING*', 'NOTICE*', 'OFL*', 'UNLICENSE*'] for p in base.glob(pattern) if p.is_file()}
        if package.get('license_file') and (base / package['license_file']).is_file():
            candidates.add(base / package['license_file'])
        for source in sorted(candidates):
            dest.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, dest / source.name)
        license_records.append({'name': package['name'], 'version': package['version'], 'license': package.get('license'), 'source': package.get('source'), 'repository': package.get('repository'), 'license_texts': [p.name for p in sorted(candidates)]})
    (output / 'sbom.json').write_text(json.dumps({'target': target, 'scope': 'selected normal/build dependencies', 'packages': license_records}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (output / 'source.json').write_text(json.dumps(before, indent=2) + '\n', encoding='utf-8')
    if system == 'macos':
        import plistlib
        contents = output / 'JoyDeck.app/Contents'
        (contents / 'MacOS').mkdir(parents=True)
        shutil.copy2(output / 'joydeck-cli', contents / 'MacOS/joydeck-cli')
        shutil.copy2(output / 'joydeck', contents / 'MacOS/JoyDeck')
        shutil.copy2(output / 'joydeck-video-macos', contents / 'MacOS/joydeck-video-macos')
        (contents / 'Info.plist').write_bytes(plistlib.dumps({
            'CFBundleExecutable': 'JoyDeck',
            'CFBundleIdentifier': 'dev.joyoffice.joydeck',
            'CFBundleName': 'JoyDeck',
            'CFBundleDisplayName': 'JoyDeck',
            'CFBundleShortVersionString': version,
            'CFBundleVersion': version,
            'CFBundlePackageType': 'APPL',
            'CFBundleIconFile': bundle_icon(contents, 'joydeck'),
            'NSHighResolutionCapable': True,
            'CFBundleDocumentTypes': [{
                'CFBundleTypeName': 'PowerPoint presentation',
                'CFBundleTypeRole': 'Editor',
                'LSHandlerRank': 'Alternate',
                'LSItemContentTypes': ['org.openxmlformats.presentationml.presentation'],
            }],
        }))
    elif system == 'linux':
        linux_launcher(output, 'joydeck')
        copy_linux_installer(output)
    # Preserve the host's SPDX/build metadata contract for release verification.
    import sbom as suite_sbom
    dependency_info = suite_sbom.generate(output / 'SBOM.spdx.json', target, args.profile,
        features=('joydeck/validation',) if args.validation else ())
    (output / 'SOURCE-HASHES.json').write_text(json.dumps(before['files'], indent=2) + '\n', encoding='utf-8')
    (output / 'BUILD-INFO.json').write_text(json.dumps({
        'version': version, 'target': target, 'profile': args.profile,
        'stage': 'R9', 'sourceSnapshotVerified': before == source_snapshot(),
        'packageRoots': ['joydeck', 'joydeck-cli'], 'officialSignatureVerified': False,
        'sourceGraphSha256': before['sha256'],
        'dependencyPackages': dependency_info['packages']}, indent=2) + '\n', encoding='utf-8')
    if before != source_snapshot():
        raise ValueError('Source changed while preparing suite metadata')
    if system == 'macos':
        resources = output / 'JoyDeck.app/Contents/Resources'
        for folder in ('assets', 'licenses', 'samples'):
            shutil.copytree(output / folder, resources / folder)
        for meta_file in ('LICENSE', 'THIRD_PARTY-JOYDECK.md', 'README.md', 'SBOM.spdx.json', 'SOURCE-HASHES.json', 'BUILD-INFO.json', 'sbom.json'):
            shutil.copy2(output / meta_file, resources / meta_file)
        build_macos_dmg('joydeck', output / 'JoyDeck.app', dmg_package)
    hashes = {p.relative_to(output).as_posix(): sha256(p) for p in sorted(output.rglob('*')) if p.is_file()}
    (output / 'SHA256SUMS').write_text(''.join(f'{value}  {key}\n' for key, value in hashes.items()), encoding='utf-8')
    create_archive(output, archive)
    write_sha256_sidecar(archive)
    if system == 'linux':
        create_tar_gz_from_directory(output, tar_archive, root_prefix=output.name)
        write_sha256_sidecar(tar_archive)
        doc_entries = [(name, output / name) for name in ('LICENSE', 'THIRD_PARTY-JOYDECK.md', 'README.md', 'SBOM.spdx.json', 'SOURCE-HASHES.json', 'BUILD-INFO.json', 'sbom.json')]
        doc_entries += [(p.relative_to(output).as_posix(), p) for p in sorted((output / 'licenses').rglob('*')) if p.is_file()]
        build_linux_deb(
            'joydeck', version, arch, deb_package,
            binary=output / 'joydeck', cli_binary=output / 'joydeck-cli',
            doc_entries=doc_entries,
        )
    report = {'directory': str(output), 'archive': str(archive), 'archive_sha256': sha256(archive), 'source_sha256': before['sha256'], 'target': target, 'profile': args.profile, 'sbom_packages': len(dependencies), 'missing_license_texts': dependency_info['missingLicenseTexts'], 'signed': False}
    if system == 'macos' and dmg_package.is_file():
        report['dmg'] = str(dmg_package)
        report['dmg_sha256'] = sha256(dmg_package)
    elif system == 'linux':
        report['tar_gz'] = str(tar_archive)
        report['tar_gz_sha256'] = sha256(tar_archive)
        if deb_package.is_file():
            report['deb'] = str(deb_package)
            report['deb_sha256'] = sha256(deb_package)
    args.evidence.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Packaging failed: {error}', file=sys.stderr)
        sys.exit(1)
