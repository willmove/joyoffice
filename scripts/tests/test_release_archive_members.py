"""Release gate validates all member paths and types without extracting them."""
import io
import stat
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

from test_verify_release_assets import assets, package


class ArchiveMembers(unittest.TestCase):
    def check(self, product, platform, entries, rejected=True):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            arch = 'aarch64' if platform == 'macos' else 'x86_64'
            package(root, product, '0.1.0', platform, arch)
            path = root / assets.filename(product, '0.1.0', platform, arch)
            prefix = path.name[:-4] + '/' if product != 'joyword' else ''
            if path.name.endswith('.zip'):
                with zipfile.ZipFile(path, 'a') as archive:
                    for name, kind in entries:
                        name = name.replace('$ROOT/', prefix)
                        item = zipfile.ZipInfo(name)
                        # Preserve the raw fixture name when ZipInfo normalizes
                        # Windows separators during construction.
                        item.filename = name
                        item.external_attr = (kind | 0o755) << 16
                        archive.writestr(item, b'' if name.endswith('/') else b'fixture')
            else:
                with tarfile.open(path, 'r:gz') as archive:
                    old = [(i, archive.extractfile(i).read()) for i in archive.getmembers()]
                with tarfile.open(path, 'w:gz') as archive:
                    for item, data in old:
                        archive.addfile(item, io.BytesIO(data))
                    for name, kind in entries:
                        item = tarfile.TarInfo(name.replace('$ROOT/', prefix))
                        item.type = kind
                        if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                            item.linkname = '../../outside'
                        archive.addfile(item)
            # A rejection must happen before attempting to read BUILD-INFO or binaries.
            with mock.patch.object(assets, 'executable_header', wraps=assets.executable_header) as header, \
                    mock.patch.object(assets.json, 'load', wraps=assets.json.load) as load:
                if rejected:
                    with self.assertRaises(ValueError):
                        assets.verify_archive(path, product, '0.1.0', platform, arch)
                    header.assert_not_called()
                    load.assert_not_called()
                else:
                    assets.verify_archive(path, product, '0.1.0', platform, arch)
                    self.assertTrue(header.called)

    def test_unsafe_paths_across_all_products_and_formats(self):
        for product in ('joyword', 'joydeck', 'joysheet'):
            for platform in ('linux', 'windows', 'macos'):
                kind = tarfile.REGTYPE if product == 'joyword' and platform == 'linux' else stat.S_IFREG
                for name in ('../escape', '/absolute', 'C:/escape', '$ROOT/../escape',
                             '$ROOT/a/../../escape', '$ROOT/a\\b', '$ROOT/a//b',
                             '$ROOT/./alias', '$ROOT/file:stream', '$ROOT/control\x01name'):
                    with self.subTest(product=product, platform=platform, name=name):
                        self.check(product, platform, [(name, kind)])

    def test_zip_links_special_files_and_type_mismatch(self):
        for product in ('joyword', 'joydeck', 'joysheet'):
            for name, kind in [('$ROOT/link', stat.S_IFLNK), ('$ROOT/fifo', stat.S_IFIFO),
                               ('$ROOT/device', stat.S_IFCHR), ('$ROOT/socket', stat.S_IFSOCK),
                               ('$ROOT/dir', stat.S_IFDIR), ('$ROOT/file/', stat.S_IFREG)]:
                with self.subTest(product=product, name=name):
                    self.check(product, 'windows', [(name, kind)])

    def test_tar_links_special_and_sparse_members(self):
        for kind in (tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE,
                     tarfile.CHRTYPE, tarfile.BLKTYPE, tarfile.GNUTYPE_SPARSE):
            with self.subTest(kind=kind):
                self.check('joyword', 'linux', [('extra', kind)])

    def test_windows_aliases_and_file_parent_collisions(self):
        for entries in [ [('CON.txt', stat.S_IFREG)], [('file.', stat.S_IFREG)],
                         [('file ', stat.S_IFREG)], [('A', stat.S_IFREG), ('a', stat.S_IFREG)],
                         [('parent', stat.S_IFREG), ('parent/child', stat.S_IFREG)],
                         [('folder/', stat.S_IFDIR), ('folder', stat.S_IFREG)]]:
            with self.subTest(entries=entries):
                self.check('joyword', 'windows', entries)

    def test_deck_and_sheet_reject_members_outside_package_root(self):
        for product in ('joydeck', 'joysheet'):
            self.check(product, 'linux', [('other/file', stat.S_IFREG)])

    def test_macos_unicode_and_case_aliases(self):
        for names in [('A', 'a'), ('é', 'e\u0301')]:
            with self.subTest(names=names):
                self.check('joyword', 'macos', [(name, stat.S_IFREG) for name in names])

    def test_zip_nul_truncation_is_rejected(self):
        item = zipfile.ZipInfo('safe\x00hidden')
        with self.assertRaisesRegex(ValueError, 'Unsafe ZIP'):
            assets.validate_members([item], True, '', 'linux')

    def test_tar_pax_override_is_checked(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'override.tar.gz'
            with tarfile.open(path, 'w:gz', format=tarfile.PAX_FORMAT) as archive:
                item = tarfile.TarInfo('safe')
                item.pax_headers = {'path': '../escape'}
                archive.addfile(item)
            with self.assertRaisesRegex(ValueError, 'Unsafe archive path'):
                assets.verify_archive(path, 'joyword', '0.1.0', 'linux', 'x86_64')

    def test_regular_files_and_explicit_directories_are_preserved(self):
        for product, platform, directory, regular in [
                ('joyword', 'linux', tarfile.DIRTYPE, tarfile.REGTYPE),
                ('joyword', 'windows', stat.S_IFDIR, 0),
                ('joydeck', 'linux', stat.S_IFDIR, stat.S_IFREG),
                ('joysheet', 'windows', stat.S_IFDIR, stat.S_IFREG)]:
            with self.subTest(product=product, platform=platform):
                self.check(product, platform, [('$ROOT/docs/', directory),
                           ('$ROOT/docs/说明 file.txt', regular)], rejected=False)
