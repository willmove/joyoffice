"""Release gate accepts complete archives and rejects missing or mislabeled binaries."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location(
    'release_assets', Path(__file__).resolve().parents[1] / 'verify_release_assets.py')
assets = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assets)


def binary(platform, arch):
    header = bytearray(128)
    if platform == 'linux':
        header[:6] = b'\x7fELF\x02\x01'
        struct.pack_into('<H', header, 18, {'x86_64': 62, 'aarch64': 183}[arch])
    elif platform == 'macos':
        header[:4] = b'\xcf\xfa\xed\xfe'
        struct.pack_into('<I', header, 4, 0x0100000c)
    else:
        header[:2] = b'MZ'
        struct.pack_into('<I', header, 60, 64)
        header[64:70] = b'PE\0\0\x64\x86'
    return bytes(header)


def package(directory, product, version, platform, arch, *, missing_cli=False, wrong_arch=False):
    name = assets.filename(product, version, platform, arch)
    path = directory / name
    root = name[:-4] + '/' if product in {'joydeck', 'joysheet'} else ''
    info = {'version': version, 'profile': 'release', 'target': assets.TARGETS[platform, arch],
            'packageRoots': [product, product + '-cli'], 'sourceSnapshotVerified': True}
    files = {root + 'BUILD-INFO.json': json.dumps(info).encode(),
             root + 'SBOM.spdx.json': b'{}', root + 'SOURCE-HASHES.json': b'{}'}
    suffix = '.exe' if platform == 'windows' else ''
    if product == 'joyword' and platform == 'macos':
        app, cli = 'JoyWord.app/Contents/MacOS/joyword', 'JoyWord.app/Contents/MacOS/joyword-cli'
    else:
        app, cli = root + product + suffix, root + product + '-cli' + suffix
    data = binary('linux', 'aarch64') if wrong_arch else binary(platform, arch)
    files[app] = data
    if not missing_cli:
        files[cli] = data
    if product == 'joydeck' and platform == 'macos':
        files[root + 'JoyDeck.app/Contents/MacOS/JoyDeck'] = data
        files[root + 'JoyDeck.app/Contents/MacOS/joydeck-cli'] = data
    if product == 'joysheet' and platform == 'macos':
        files[root + 'Joysheet.app/Contents/MacOS/joysheet'] = data
        files[root + 'Joysheet.app/Contents/MacOS/joysheet-cli'] = data
    if name.endswith('.zip'):
        with zipfile.ZipFile(path, 'w') as archive:
            for key, value in files.items():
                item = zipfile.ZipInfo(key)
                item.external_attr = 0o100755 << 16
                archive.writestr(item, value)
    else:
        with tarfile.open(path, 'w:gz') as archive:
            for key, value in files.items():
                item = tarfile.TarInfo(key)
                item.size = len(value)
                item.mode = 0o755
                archive.addfile(item, io.BytesIO(value))
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    (directory / (name + '.sha256')).write_text(digest + '  ' + name + '\n', encoding='ascii')


class ReleaseAssets(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def matrix(self, product, **override):
        for platform, arch in assets.MATRIX:
            kwargs = override if (platform, arch) == ('linux', 'x86_64') else {}
            package(self.directory, product, '0.1.0', platform, arch, **kwargs)

    def test_all_products_accept_complete_platform_matrices(self):
        for product in ('joyword', 'joydeck', 'joysheet'):
            with self.subTest(product=product), tempfile.TemporaryDirectory() as tmp:
                self.directory = Path(tmp)
                self.matrix(product)
                assets.verify_assets(self.directory, product, '0.1.0')

    def test_rejects_missing_cli(self):
        self.matrix('joyword', missing_cli=True)
        with self.assertRaisesRegex(ValueError, 'missing file joyword-cli'):
            assets.verify_assets(self.directory, 'joyword', '0.1.0')

    def test_rejects_mislabeled_architecture(self):
        self.matrix('joydeck', wrong_arch=True)
        with self.assertRaisesRegex(ValueError, 'invalid linux x86_64 executable'):
            assets.verify_assets(self.directory, 'joydeck', '0.1.0')

    def test_sheet_cannot_publish_a_package_without_its_cli(self):
        self.matrix('joysheet', missing_cli=True)
        with self.assertRaisesRegex(ValueError, 'missing file .*joysheet-cli'):
            assets.verify_assets(self.directory, 'joysheet', '0.1.0')

    def test_rejects_missing_platform_and_bad_checksum(self):
        self.matrix('joydeck')
        name = assets.filename('joydeck', '0.1.0', 'macos', 'aarch64')
        sidecar = self.directory / (name + '.sha256')
        sidecar.unlink()
        with self.assertRaisesRegex(ValueError, 'asset set differs'):
            assets.verify_assets(self.directory, 'joydeck', '0.1.0')
        sidecar.write_text('bad  ' + name, encoding='ascii')
        with self.assertRaisesRegex(ValueError, 'SHA-256 sidecar'):
            assets.verify_assets(self.directory, 'joydeck', '0.1.0')


if __name__ == '__main__':
    unittest.main()
