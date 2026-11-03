"""A registry license may have an epoch timestamp; packages must still be valid."""
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('package_native', Path(__file__).resolve().parents[1] / 'joydeck/package-native.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class PackageArchive(unittest.TestCase):
    def test_epoch_license_survives_packaging(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'joydeck-test'
            source.mkdir()
            license_file = source / 'LICENSE'
            license_file.write_text('license test', encoding='utf-8')
            os.utime(license_file, (1, 1))
            archive = root / 'joydeck-test.zip'
            package.create_archive(source, archive)
            with zipfile.ZipFile(archive) as bundle:
                self.assertEqual(bundle.read('joydeck-test/LICENSE'), b'license test')
                self.assertIsNone(bundle.testzip())
            self.assertFalse(archive.with_name(archive.name + '.partial').exists())

    def test_existing_archive_is_never_replaced(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'joydeck-test'
            source.mkdir()
            archive = root / 'joydeck-test.zip'
            archive.write_bytes(b'existing')
            with self.assertRaises(FileExistsError):
                package.create_archive(source, archive)
            self.assertEqual(archive.read_bytes(), b'existing')
