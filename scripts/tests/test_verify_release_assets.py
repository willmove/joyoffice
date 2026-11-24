"""Release gate accepts complete aligned archives/installers and rejects UOS/ABI regressions."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import tarfile
import tempfile
import unittest
import zipfile

SCRIPTS = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('release_assets', SCRIPTS / 'verify_release_assets.py')
assets = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(assets)

INST_SPEC = importlib.util.spec_from_file_location('package_installers', SCRIPTS / 'package_installers.py')
installers = importlib.util.module_from_spec(INST_SPEC)
INST_SPEC.loader.exec_module(installers)


def binary(platform, arch, *, glibc_versions=()):
    if platform == 'linux' and glibc_versions:
        return elf_with_glibc(arch, glibc_versions)
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


def elf_with_glibc(arch, versions):
    """Build a minimal valid 64-bit ELF with SHT_GNU_verneed entries for ABI testing."""
    machine = {'x86_64': 62, 'aarch64': 183}[arch]
    strtab = bytearray(b'\0libc.so.6\0')
    offsets = []
    for name in versions:
        offsets.append(len(strtab))
        strtab.extend(name.encode('ascii') + b'\0')
    # Elf64_Verneed (16 bytes) + N * Elf64_Vernaux (16 bytes)
    verneed = bytearray(struct.pack('<HHIII', 1, len(offsets), 1, 16, 0))
    for idx, off in enumerate(offsets):
        nxt = 16 if idx + 1 < len(offsets) else 0
        verneed.extend(struct.pack('<IHHII', 0, 0, idx + 2, off, nxt))
    ehdr_size = 64
    str_off = ehdr_size
    vn_off = str_off + len(strtab)
    sh_off = vn_off + len(verneed)
    ehdr = bytearray(ehdr_size)
    ehdr[:6] = b'\x7fELF\x02\x01'
    struct.pack_into('<H', ehdr, 18, machine)
    struct.pack_into('<Q', ehdr, 40, sh_off)
    struct.pack_into('<HH', ehdr, 58, 64, 3)
    sh_null = bytes(64)
    sh_str = struct.pack('<IIQQQQIIQQ', 0, 3, 0, 0, str_off, len(strtab), 0, 0, 1, 0)
    sh_vn = struct.pack('<IIQQQQIIQQ', 0, 0x6ffffffe, 0, 0, vn_off, len(verneed), 1, 1, 8, 0)
    return bytes(ehdr + strtab + verneed + sh_null + sh_str + sh_vn)


def write_archive(directory, name, files):
    path = directory / name
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


def package(directory, product, version, platform, arch, *,
            missing_cli=False, wrong_arch=False, wrong_icon=False, wrong_desktop_installer=False,
            stale_system_icon=False, glibc_versions=()):
    stem = assets.archive_stem(product, version, platform, arch)
    root = stem + '/' if product in {'joydeck', 'joysheet'} else ''
    info = {'version': version, 'profile': 'release', 'target': assets.TARGETS[platform, arch],
            'packageRoots': [product, product + '-cli'], 'sourceSnapshotVerified': True}
    files = {root + 'BUILD-INFO.json': json.dumps(info).encode(),
             root + 'SBOM.spdx.json': b'{}', root + 'SOURCE-HASHES.json': b'{}'}
    suffix = '.exe' if platform == 'windows' else ''
    if product == 'joyword' and platform == 'macos':
        app, cli = 'JoyWord.app/Contents/MacOS/joyword', 'JoyWord.app/Contents/MacOS/joyword-cli'
    else:
        app, cli = root + product + suffix, root + product + '-cli' + suffix
    data = binary('linux', 'aarch64') if wrong_arch else binary(platform, arch, glibc_versions=glibc_versions)
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
        installer = Path(assets.__file__).with_name('install_linux_desktop.py').read_bytes()
        files[root + 'install-desktop.py'] = b'old installer' if wrong_desktop_installer else installer
        prefixes = ('Joysheet.AppDir/usr/', 'debian/usr/') if product == 'joysheet' else ('',)
        for prefix in prefixes:
            share = root + prefix + 'share/'
            files[share + 'icons/hicolor/scalable/apps/' + logo] = b'old icon' if wrong_icon else svg
            icon = f'/usr/share/icons/hicolor/scalable/apps/{logo}' if prefix == 'debian/usr/' and not stale_system_icon else product
            files[share + f'applications/{product}.desktop'] = (
                f'[Desktop Entry]\nIcon={icon}\nExec={product} %f\nStartupWMClass={product}\n'
            ).encode()
    for name in assets.portable_filenames(product, version, platform, arch):
        write_archive(directory, name, files)


class ReleaseAssets(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def matrix(self, product, **override):
        for platform, arch in assets.MATRIX:
            kwargs = override if (platform, arch) == ('linux', 'x86_64') else {}
            package(self.directory, product, '0.3.0', platform, arch, **kwargs)
            name = assets.installer_filename(product, '0.3.0', platform, arch)
            installer = self.directory / name
            installer.write_bytes(b'installer fixture')
            (self.directory / (name + '.sha256')).write_text(
                f'{assets.digest_file(installer)}  {name}\n', encoding='ascii')

    def test_all_products_accept_complete_aligned_platform_matrices(self):
        for product in ('joyword', 'joydeck', 'joysheet'):
            with self.subTest(product=product), tempfile.TemporaryDirectory() as tmp:
                self.directory = Path(tmp)
                self.matrix(product)
                self.assertEqual(len(assets.expected_files(product, '0.3.0')), 28)
                assets.verify_assets(self.directory, product, '0.3.0')

    def test_all_products_require_macos_dmg_linux_deb_and_both_linux_archives(self):
        for product in ('joyword', 'joydeck', 'joysheet'):
            for target_name in (
                assets.installer_filename(product, '0.3.0', 'macos', 'aarch64'),
                assets.installer_filename(product, '0.3.0', 'linux', 'aarch64'),
                assets.installer_filename(product, '0.3.0', 'windows', 'aarch64'),
                assets.portable_filenames(product, '0.3.0', 'linux', 'x86_64')[1],
            ):
                with self.subTest(product=product, missing=target_name), tempfile.TemporaryDirectory() as tmp:
                    self.directory = Path(tmp)
                    self.matrix(product)
                    (self.directory / target_name).unlink()
                    with self.assertRaisesRegex(ValueError, 'asset set differs'):
                        assets.verify_assets(self.directory, product, '0.3.0')

    def test_rejects_missing_cli(self):
        self.matrix('joyword', missing_cli=True)
        with self.assertRaisesRegex(ValueError, 'missing file joyword-cli'):
            assets.verify_assets(self.directory, 'joyword', '0.3.0')

    def test_rejects_mislabeled_architecture(self):
        self.matrix('joydeck', wrong_arch=True)
        with self.assertRaisesRegex(ValueError, 'invalid linux x86_64 executable'):
            assets.verify_assets(self.directory, 'joydeck', '0.3.0')

    def test_rejects_glibc_newer_than_uos_baseline_and_accepts_2_28(self):
        self.matrix('joyword', glibc_versions=('GLIBC_2.17', 'GLIBC_2.28'))
        assets.verify_assets(self.directory, 'joyword', '0.3.0')
        with tempfile.TemporaryDirectory() as tmp:
            self.directory = Path(tmp)
            self.matrix('joyword', glibc_versions=('GLIBC_2.17', 'GLIBC_2.39'))
            with self.assertRaisesRegex(ValueError, 'exceeding UOS baseline GLIBC_2.28'):
                assets.verify_assets(self.directory, 'joyword', '0.3.0')

    def test_rejects_stale_platform_icon_even_with_valid_archive_checksum(self):
        self.matrix('joydeck', wrong_icon=True)
        with self.assertRaisesRegex(ValueError, 'Incorrect product icon'):
            assets.verify_assets(self.directory, 'joydeck', '0.3.0')

    def test_rejects_stale_linux_desktop_installer(self):
        self.matrix('joyword', wrong_desktop_installer=True)
        with self.assertRaisesRegex(ValueError, 'Incorrect desktop installer'):
            assets.verify_assets(self.directory, 'joyword', '0.3.0')

    def test_rejects_theme_name_only_icon_in_system_installation(self):
        self.matrix('joysheet', stale_system_icon=True)
        with self.assertRaisesRegex(ValueError, 'Incorrect desktop icon or launcher reference'):
            assets.verify_assets(self.directory, 'joysheet', '0.3.0')

    def test_sheet_cannot_publish_a_package_without_its_cli(self):
        self.matrix('joysheet', missing_cli=True)
        with self.assertRaisesRegex(ValueError, 'missing file .*joysheet-cli'):
            assets.verify_assets(self.directory, 'joysheet', '0.3.0')

    def test_rejects_missing_platform_and_bad_checksum(self):
        self.matrix('joydeck')
        name = assets.filename('joydeck', '0.3.0', 'macos', 'aarch64')
        sidecar = self.directory / (name + '.sha256')
        sidecar.unlink()
        with self.assertRaisesRegex(ValueError, 'asset set differs'):
            assets.verify_assets(self.directory, 'joydeck', '0.3.0')
        sidecar.write_text('bad  ' + name, encoding='ascii')
        with self.assertRaisesRegex(ValueError, 'SHA-256 sidecar'):
            assets.verify_assets(self.directory, 'joydeck', '0.3.0')

    @unittest.skipUnless(shutil.which('dpkg-deb') and shutil.which('ar'), 'requires dpkg-deb and ar')
    def test_linux_deb_builder_and_verifier_cover_all_products_and_arm64(self):
        for product in ('joyword', 'joydeck', 'joysheet'):
            for arch in ('x86_64', 'aarch64'):
                with self.subTest(product=product, arch=arch), tempfile.TemporaryDirectory() as tmp:
                    root = Path(tmp)
                    app = root / product
                    cli = root / (product + '-cli')
                    app.write_bytes(binary('linux', arch, glibc_versions=('GLIBC_2.17', 'GLIBC_2.28')))
                    cli.write_bytes(binary('linux', arch, glibc_versions=('GLIBC_2.17', 'GLIBC_2.28')))
                    app.chmod(0o755)
                    cli.chmod(0o755)
                    lic = root / 'LICENSE'
                    lic.write_text('MIT', encoding='utf-8')
                    deb = root / assets.installer_filename(product, '0.3.0', 'linux', arch)
                    installers.build_linux_deb(
                        product, '0.3.0', arch, deb,
                        binary=app, cli_binary=cli, doc_entries=[('LICENSE', lic)],
                    )
                    assets.verify_deb(deb, product, '0.3.0', arch)

    @unittest.skipUnless(shutil.which('dpkg-deb') and shutil.which('ar'), 'requires dpkg-deb and ar')
    def test_verify_deb_rejects_glibc_2_39_binary_and_ubuntu_only_depends(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            app = root / 'joyword'
            cli = root / 'joyword-cli'
            app.write_bytes(binary('linux', 'aarch64', glibc_versions=('GLIBC_2.17', 'GLIBC_2.39')))
            cli.write_bytes(binary('linux', 'aarch64', glibc_versions=('GLIBC_2.17', 'GLIBC_2.28')))
            app.chmod(0o755)
            cli.chmod(0o755)
            deb = root / assets.installer_filename('joyword', '0.3.0', 'linux', 'aarch64')
            installers.build_linux_deb('joyword', '0.3.0', 'aarch64', deb, binary=app, cli_binary=cli)
            with self.assertRaisesRegex(ValueError, 'exceeding UOS baseline GLIBC_2.28'):
                assets.verify_deb(deb, 'joyword', '0.3.0', 'aarch64')
            bad_stage = root / 'bad'
            (bad_stage / 'DEBIAN').mkdir(parents=True)
            (bad_stage / 'DEBIAN/control').write_text(
                'Package: joyword\nVersion: 0.3.0\nArchitecture: arm64\nMaintainer: test\n'
                'Depends: libc6 (>= 2.39), libssl3t64\nDescription: bad\n', encoding='utf-8')
            bad_deb = root / 'bad.deb'
            subprocess.run(['dpkg-deb', '-Zxz', '--build', '--root-owner-group', str(bad_stage), str(bad_deb)], check=True)
            with self.assertRaisesRegex(ValueError, 'incompatible with UOS 20/25'):
                assets.verify_deb(bad_deb, 'joyword', '0.3.0', 'aarch64')


if __name__ == '__main__':
    unittest.main()
