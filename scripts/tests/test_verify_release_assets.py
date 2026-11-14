"""Release gate accepts complete archives and rejects missing or mislabeled binaries."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import plistlib
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
        struct.pack_into('<I', header, 4, {'x86_64': 0x01000007, 'aarch64': 0x0100000c}[arch])
    else:
        header[:2] = b'MZ'
        struct.pack_into('<I', header, 60, 64)
        header[64:68] = b'PE\0\0'
        struct.pack_into('<H', header, 68, {'x86_64': 0x8664, 'aarch64': 0xaa64}[arch])
    return bytes(header)


def package(directory, product, version, platform, arch, *,
            missing_cli=False, wrong_arch=False, wrong_icon=False):
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
    logo = f'{product}.svg'
    svg = (assets.BRAND / logo).read_bytes()
    files[root + 'assets/brand/' + logo] = svg
    if platform == 'macos':
        contents = root + f'{assets.MAC_APPS[product]}.app/Contents/'
        icon = f'{product}.icns'
        files[contents + 'Info.plist'] = plistlib.dumps({'CFBundleIconFile': icon})
        files[contents + 'Resources/' + icon] = (assets.BRAND / icon).read_bytes()
    elif platform == 'linux':
        prefixes = ('Joysheet.AppDir/usr/', 'debian/usr/') if product == 'joysheet' else ('',)
        for prefix in prefixes:
            share = root + prefix + 'share/'
            files[share + 'icons/hicolor/scalable/apps/' + logo] = b'old icon' if wrong_icon else svg
            files[share + f'applications/{product}.desktop'] = (
                f'[Desktop Entry]\nIcon={product}\nExec={product} %f\nStartupWMClass={product}\n'
            ).encode()
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
            if product == 'joysheet' or platform == 'windows':
                name = assets.installer_filename(product, '0.1.0', platform, arch)
                installer = self.directory / name
                installer.write_bytes(b'installer fixture')
                (self.directory / (name + '.sha256')).write_text(
                    f'{assets.digest_file(installer)}  {name}\n', encoding='ascii')

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

    def test_rejects_stale_platform_icon_even_with_valid_archive_checksum(self):
        self.matrix('joydeck', wrong_icon=True)
        with self.assertRaisesRegex(ValueError, 'Incorrect product icon'):
            assets.verify_assets(self.directory, 'joydeck', '0.1.0')

    def test_sheet_cannot_publish_a_package_without_its_cli(self):
        self.matrix('joysheet', missing_cli=True)
        with self.assertRaisesRegex(ValueError, 'missing file .*joysheet-cli'):
            assets.verify_assets(self.directory, 'joysheet', '0.1.0')

    def test_sheet_cannot_publish_a_missing_or_corrupted_installer(self):
        self.matrix('joysheet')
        installer = self.directory / assets.installer_filename('joysheet', '0.1.0', 'windows', 'aarch64')
        installer.write_bytes(b'corrupted')
        with self.assertRaisesRegex(ValueError, 'setup.exe: SHA-256 sidecar'):
            assets.verify_assets(self.directory, 'joysheet', '0.1.0')
        installer.unlink()
        with self.assertRaisesRegex(ValueError, 'asset set differs'):
            assets.verify_assets(self.directory, 'joysheet', '0.1.0')

    def test_word_and_deck_require_both_windows_installers(self):
        for product in ('joyword', 'joydeck'):
            with self.subTest(product=product), tempfile.TemporaryDirectory() as tmp:
                self.directory = Path(tmp)
                self.matrix(product)
                name = assets.installer_filename(product, '0.1.0', 'windows', 'aarch64')
                (self.directory / name).unlink()
                with self.assertRaisesRegex(ValueError, 'asset set differs'):
                    assets.verify_assets(self.directory, product, '0.1.0')

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
