#!/usr/bin/env python3
"""Verify the complete, product-specific release matrix before publication."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys
import tarfile
import zipfile


MATRIX = (('linux', 'x86_64'), ('linux', 'aarch64'),
          ('macos', 'aarch64'), ('windows', 'x86_64'))
TARGETS = {('linux', 'x86_64'): 'x86_64-unknown-linux-gnu',
           ('linux', 'aarch64'): 'aarch64-unknown-linux-gnu',
           ('macos', 'aarch64'): 'aarch64-apple-darwin',
           ('windows', 'x86_64'): 'x86_64-pc-windows-msvc'}


def filename(product, version, platform, arch):
    suffix = '-release.zip' if product in {'joydeck', 'joysheet'} else (
        '.tar.gz' if platform == 'linux' else '.zip')
    return f'{product}-{version}-{platform}-{arch}{suffix}'


def expected_files(product, version):
    assets = [filename(product, version, *pair) for pair in MATRIX]
    return set(assets + [name + '.sha256' for name in assets])


def executable_header(stream, platform, arch):
    header = stream.read(64)
    if platform == 'linux':
        machine = {'x86_64': 62, 'aarch64': 183}[arch]
        valid = (len(header) >= 20 and header[:6] == b'\x7fELF\x02\x01'
                 and struct.unpack_from('<H', header, 18)[0] == machine)
    elif platform == 'macos':
        valid = (len(header) >= 8 and header[:4] == b'\xcf\xfa\xed\xfe'
                 and struct.unpack_from('<I', header, 4)[0] == 0x0100000c)
    else:
        valid = False
        if len(header) >= 64 and header[:2] == b'MZ':
            offset = struct.unpack_from('<I', header, 60)[0]
            if offset <= 1024 * 1024:
                stream.seek(offset)
                pe = stream.read(6)
                valid = pe == b'PE\0\0\x64\x86'
    if not valid:
        raise ValueError(f'invalid {platform} {arch} executable header')


def verify_archive(path, product, version, platform, arch):
    is_zip = path.name.endswith('.zip')
    archive = zipfile.ZipFile(path) if is_zip else tarfile.open(path, 'r:gz')
    with archive:
        members = archive.infolist() if is_zip else archive.getmembers()
        names = [item.filename if is_zip else item.name for item in members]
        if len(names) != len(set(names)):
            raise ValueError(f'{path.name}: duplicate archive entries')
        index = dict(zip(names, members))

        def read_file(name):
            item = index.get(name)
            if item is None or (not is_zip and not item.isfile()) or (is_zip and item.is_dir()):
                raise ValueError(f'{path.name}: missing file {name}')
            stream = archive.open(item) if is_zip else archive.extractfile(item)
            if stream is None:
                raise ValueError(f'{path.name}: unable to read {name}')
            return stream

        root = path.name[:-4] + '/' if product in {'joydeck', 'joysheet'} else ''
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


def verify_assets(directory, product, version):
    expected = expected_files(product, version)
    actual = {p.name for p in directory.iterdir() if p.is_file()}
    if actual != expected:
        raise ValueError(f'asset set differs: missing={sorted(expected - actual)}, unexpected={sorted(actual - expected)}')
    for platform, arch in MATRIX:
        path = directory / filename(product, version, platform, arch)
        with path.open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        sidecar = (directory / (path.name + '.sha256')).read_text(encoding='ascii').split()
        if sidecar != [digest, path.name]:
            raise ValueError(f'{path.name}: SHA-256 sidecar does not match')
        verify_archive(path, product, version, platform, arch)
        print(f'Verified {path.name}: {digest}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('product', choices=['joyword', 'joydeck', 'joysheet'])
    parser.add_argument('version')
    args = parser.parse_args()
    verify_assets(args.directory, args.product, args.version)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, json.JSONDecodeError, zipfile.BadZipFile, tarfile.TarError) as error:
        sys.exit(str(error))
