#!/usr/bin/env python3
"""Verify the complete, product-specific release matrix before publication."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import stat
import struct
import subprocess
import unicodedata
import sys
import tarfile
import zipfile


TARGETS = {('linux', 'x86_64'): 'x86_64-unknown-linux-gnu',
           ('linux', 'aarch64'): 'aarch64-unknown-linux-gnu',
           ('macos', 'aarch64'): 'aarch64-apple-darwin',
           ('macos', 'x86_64'): 'x86_64-apple-darwin',
           ('windows', 'x86_64'): 'x86_64-pc-windows-msvc',
           ('windows', 'aarch64'): 'aarch64-pc-windows-msvc'}
MATRIX = tuple(TARGETS)
BRAND = Path(__file__).resolve().parents[1] / 'assets/brand'
MAC_APPS = {'joyword': 'JoyWord', 'joydeck': 'JoyDeck', 'joysheet': 'Joysheet'}


def filename(product, version, platform, arch):
    suffix = '-release.zip' if product in {'joydeck', 'joysheet'} else (
        '.tar.gz' if platform == 'linux' else '.zip')
    return f'{product}-{version}-{platform}-{arch}{suffix}'


def installer_filename(product, version, platform, arch):
    if product != 'joysheet' and platform != 'windows':
        raise ValueError('Only JoySheet has macOS/Linux installers')
    suffix = {'windows': '-setup.exe', 'macos': '.dmg', 'linux': '.deb'}[platform]
    release = '' if product == 'joyword' else '-release'
    return f'{product}-{version}-{platform}-{arch}{release}{suffix}'


def expected_files(product, version):
    assets = [filename(product, version, *pair) for pair in MATRIX]
    assets += [installer_filename(product, version, *pair) for pair in MATRIX
               if product == 'joysheet' or pair[0] == 'windows']
    return set(assets + [name + '.sha256' for name in assets])


def executable_header(stream, platform, arch):
    header = stream.read(64)
    if platform == 'linux':
        machine = {'x86_64': 62, 'aarch64': 183}[arch]
        valid = (len(header) >= 20 and header[:6] == b'\x7fELF\x02\x01'
                 and struct.unpack_from('<H', header, 18)[0] == machine)
    elif platform == 'macos':
        cpu_type = {'x86_64': 0x01000007, 'aarch64': 0x0100000c}[arch]
        valid = (len(header) >= 8 and header[:4] == b'\xcf\xfa\xed\xfe'
                 and struct.unpack_from('<I', header, 4)[0] == cpu_type)
    else:
        machine = {'x86': 0x014c, 'x86_64': 0x8664, 'aarch64': 0xaa64}[arch]
        valid = False
        if len(header) >= 64 and header[:2] == b'MZ':
            offset = struct.unpack_from('<I', header, 60)[0]
            if offset <= 1024 * 1024:
                stream.seek(offset)
                pe = stream.read(6)
                valid = pe == b'PE\0\0' + struct.pack('<H', machine)
    if not valid:
        raise ValueError(f'invalid {platform} {arch} executable header')


def validate_members(members, is_zip, root, platform):
    """Validate names/types before opening any member; never extract to disk."""
    paths = {}
    for item in members:
        name = item.filename if is_zip else item.name
        if is_zip:
            kind = stat.S_IFMT(item.external_attr >> 16)
            directory = item.is_dir()
            if (item.orig_filename != name or kind not in (0, stat.S_IFREG, stat.S_IFDIR)
                    or (kind == stat.S_IFDIR and not directory)
                    or (kind == stat.S_IFREG and directory)):
                raise ValueError('Unsafe ZIP member type or name: ' + repr(name))
        else:
            directory = item.isdir()
            if item.type not in (tarfile.REGTYPE, tarfile.AREGTYPE, tarfile.DIRTYPE) or item.sparse is not None:
                raise ValueError('Unsafe TAR member type: ' + repr(name))
        # Strip only the one optional directory terminator, not repeated slashes.
        normalized = name[:-1] if directory and name.endswith('/') else name
        parts = normalized.split('/')
        if (not normalized or any(c in name for c in '\\:')
                or any(ord(c) < 32 or ord(c) == 127 for c in name)
                or any(p in ('', '.', '..') for p in parts)):
            raise ValueError('Unsafe archive path: ' + repr(name))
        if root and normalized != root.rstrip('/') and not normalized.startswith(root):
            raise ValueError('Archive member outside package root: ' + repr(name))
        if root and normalized == root.rstrip('/') and not directory:
            raise ValueError('Package root must be a directory')
        if platform == 'windows':
            devices = {'CON', 'PRN', 'AUX', 'NUL', 'CONIN$', 'CONOUT$'}
            devices.update(prefix + suffix for prefix in ('COM', 'LPT') for suffix in '123456789¹²³')
            if any(p.endswith(('.', ' ')) or any(c in p for c in '<>"|?*')
                   or p.split('.')[0].upper() in devices for p in parts):
                raise ValueError('Unsafe Windows archive path: ' + repr(name))
        key = unicodedata.normalize('NFC', normalized).casefold() if platform != 'linux' else normalized
        if key in paths:
            raise ValueError('Duplicate or aliased archive path: ' + repr(name))
        paths[key] = directory
    for name in paths:
        parts = name.split('/')
        if any(paths.get('/'.join(parts[:i])) is False for i in range(1, len(parts))):
            raise ValueError('Archive file used as parent directory: ' + repr(name))


def verify_archive(path, product, version, platform, arch):
    is_zip = path.name.endswith('.zip')
    archive = zipfile.ZipFile(path) if is_zip else tarfile.open(path, 'r:gz')
    with archive:
        members = archive.infolist() if is_zip else archive.getmembers()
        names = [item.filename if is_zip else item.name for item in members]
        if len(names) != len(set(names)):
            raise ValueError(f'{path.name}: duplicate archive entries')
        root = path.name[:-4] + '/' if product in {'joydeck', 'joysheet'} else ''
        validate_members(members, is_zip, root, platform)
        index = dict(zip(names, members))

        def read_file(name):
            item = index.get(name)
            if item is None or (not is_zip and not item.isfile()) or (is_zip and item.is_dir()):
                raise ValueError(f'{path.name}: missing file {name}')
            stream = archive.open(item) if is_zip else archive.extractfile(item)
            if stream is None:
                raise ValueError(f'{path.name}: unable to read {name}')
            return stream

        with read_file(root + 'BUILD-INFO.json') as stream:
            info = json.load(stream)
        if (info.get('version') != version or info.get('target') != TARGETS[platform, arch]
                or info.get('profile') != 'release' or not info.get('sourceSnapshotVerified')
                or set(info.get('packageRoots', [])) != {product, product + '-cli'}):
            raise ValueError(f'{path.name}: inconsistent build metadata')
        extension = '.exe' if platform == 'windows' else ''
        if product == 'joyword' and platform == 'macos':
            executables = ['JoyWord.app/Contents/MacOS/joyword',
                           'JoyWord.app/Contents/MacOS/joyword-cli']
        else:
            executables = [root + product + extension, root + product + '-cli' + extension]
            if product == 'joydeck' and platform == 'macos':
                executables += [root + 'JoyDeck.app/Contents/MacOS/JoyDeck',
                                root + 'JoyDeck.app/Contents/MacOS/joydeck-cli']
            if product == 'joysheet' and platform == 'macos':
                executables += [root + 'Joysheet.app/Contents/MacOS/joysheet',
                                root + 'Joysheet.app/Contents/MacOS/joysheet-cli']
        for name in executables:
            with read_file(name) as stream:
                executable_header(stream, platform, arch)
            if platform != 'windows':
                item = index[name]
                mode = (item.external_attr >> 16) if is_zip else item.mode
                if not mode & 0o111:
                    raise ValueError(f'{path.name}: executable permission missing: {name}')
        for name in (root + 'SBOM.spdx.json', root + 'SOURCE-HASHES.json'):
            with read_file(name) as stream:
                json.load(stream)
        verify_branding(read_file, root, product, platform)


def verify_branding(read_file, root, product, platform):
    def match_asset(member, source):
        expected = (BRAND / source).read_bytes()
        with read_file(member) as stream:
            if stream.read(len(expected) + 1) != expected:
                raise ValueError(f'Incorrect product icon: {member}')

    logo = f'{product}.svg'
    match_asset(root + 'assets/brand/' + logo, logo)
    if platform == 'macos':
        contents = root + f'{MAC_APPS[product]}.app/Contents/'
        icon = f'{product}.icns'
        with read_file(contents + 'Info.plist') as stream:
            if plistlib.load(stream).get('CFBundleIconFile') != icon:
                raise ValueError(f'Incorrect app icon reference: {contents}Info.plist')
        match_asset(contents + 'Resources/' + icon, icon)
    elif platform == 'linux':
        prefixes = ('Joysheet.AppDir/usr/', 'debian/usr/') if product == 'joysheet' else ('',)
        for prefix in prefixes:
            share = root + prefix + 'share/'
            match_asset(share + 'icons/hicolor/scalable/apps/' + logo, logo)
            desktop = share + f'applications/{product}.desktop'
            with read_file(desktop) as stream:
                fields = stream.read(4096).decode('utf-8').splitlines()
            required = {f'Icon={product}', f'Exec={product} %f', f'StartupWMClass={product}'}
            if not required.issubset(fields):
                raise ValueError(f'Incorrect desktop icon or launcher reference: {desktop}')


def digest_file(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def prepare_installer(directory, product, version, platform, arch):
    path = directory / installer_filename(product, version, platform, arch)
    if platform == 'macos':
        subprocess.run(['hdiutil', 'verify', str(path)], check=True)
    elif platform == 'linux':
        control = subprocess.check_output(['dpkg-deb', '--field', str(path)], text=True)
        deb_arch = {'x86_64': 'amd64', 'aarch64': 'arm64'}[arch]
        if not {'Package: joysheet', f'Version: {version}', f'Architecture: {deb_arch}'}.issubset(control.splitlines()):
            raise ValueError(f'{path.name}: inconsistent Debian package metadata')
    else:
        # NSIS uses a 32-bit bootstrapper for both payload architectures.
        with path.open('rb') as stream:
            executable_header(stream, platform, 'x86')
    path.with_name(path.name + '.sha256').write_text(
        f'{digest_file(path)}  {path.name}\n', encoding='ascii')


def verify_assets(directory, product, version):
    expected = expected_files(product, version)
    actual = {p.name for p in directory.iterdir() if p.is_file()}
    if actual != expected:
        raise ValueError(f'asset set differs: missing={sorted(expected - actual)}, unexpected={sorted(actual - expected)}')
    for name in sorted(expected):
        if name.endswith('.sha256'):
            continue
        path = directory / name
        digest = digest_file(path)
        sidecar = (directory / (path.name + '.sha256')).read_text(encoding='ascii').split()
        if sidecar != [digest, path.name]:
            raise ValueError(f'{path.name}: SHA-256 sidecar does not match')
        print(f'Verified checksum for {path.name}: {digest}')
    for platform, arch in MATRIX:
        path = directory / filename(product, version, platform, arch)
        verify_archive(path, product, version, platform, arch)
        print(f'Verified archive contents for {path.name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('product', choices=['joyword', 'joydeck', 'joysheet'])
    parser.add_argument('version')
    parser.add_argument('--prepare-installer', nargs=2, metavar=('PLATFORM', 'ARCH'))
    args = parser.parse_args()
    if args.prepare_installer:
        pair = tuple(args.prepare_installer)
        if pair not in MATRIX or (args.product != 'joysheet' and pair[0] != 'windows'):
            parser.error('Installer preparation requires a supported product/platform/architecture')
        prepare_installer(args.directory, args.product, args.version, *pair)
    else:
        verify_assets(args.directory, args.product, args.version)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError, zipfile.BadZipFile, tarfile.TarError) as error:
        sys.exit(str(error))
