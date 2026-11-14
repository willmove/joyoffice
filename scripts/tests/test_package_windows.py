"""Installer input, architecture and file-safe uninstall contracts."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package_windows as installer
from test_verify_release_assets import binary, package


class WindowsInstaller(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def payload(self, product='joyword', arch='x86_64'):
        info = {'version': '0.1.4', 'target': installer.info_target(arch),
                'sourceSnapshotVerified': True, 'packageRoots': [product, product + '-cli']}
        (self.directory / 'BUILD-INFO.json').write_text(json.dumps(info), encoding='utf-8')
        for name in (product, product + '-cli'):
            (self.directory / (name + '.exe')).write_bytes(binary('windows', arch))
        return info

    def test_payload_requires_matching_gui_cli_and_target(self):
        self.payload()
        self.assertEqual(installer.payload_info(self.directory, 'joyword'), ('0.1.4', 'x86_64'))
        (self.directory / 'joyword-cli.exe').write_bytes(binary('windows', 'aarch64'))
        with self.assertRaisesRegex(ValueError, 'invalid windows x86_64'):
            installer.payload_info(self.directory, 'joyword')

    def test_unverified_or_other_product_metadata_is_rejected(self):
        info = self.payload()
        for change in ({'sourceSnapshotVerified': False}, {'packageRoots': ['joydeck', 'joydeck-cli']},
                       {'version': '0.1.4\n!system malicious'}):
            with self.subTest(change=change):
                (self.directory / 'BUILD-INFO.json').write_text(json.dumps(info | change), encoding='utf-8')
                with self.assertRaises(ValueError):
                    installer.payload_info(self.directory, 'joyword')

    def test_uninstall_inventory_deletes_only_known_files(self):
        payload = self.directory / 'payload'
        (payload / 'nested').mkdir(parents=True)
        (payload / 'nested' / 'file$0.txt').write_text('data')
        output = self.directory / 'uninstall.nsh'
        installer.write_uninstall_inventory(payload, output)
        self.assertEqual(output.read_text(), 'Delete "$INSTDIR\\nested\\file$$0.txt"\nRMDir "$INSTDIR\\nested"\n')
        self.assertNotIn('/r', output.read_text().lower())

    def test_nsis_values_escape_variables_and_reject_instructions(self):
        self.assertEqual(installer.nsis_literal('a$0"b'), 'a$$0$\\"b')
        with self.assertRaises(ValueError):
            installer.nsis_literal('file\nDelete *')

    def test_archive_requires_original_checksum_before_compilation(self):
        package(self.directory, 'joyword', '0.1.4', 'windows', 'x86_64')
        archive = self.directory / 'joyword-0.1.4-windows-x86_64.zip'
        archive.with_name(archive.name + '.sha256').write_text('0' * 64 + '  ' + archive.name)
        with patch.object(installer, 'compile_payload') as compile_mock:
            with self.assertRaisesRegex(ValueError, 'SHA-256 sidecar'):
                installer.package_archive(archive, 'joyword', '0.1.4', 'x86_64',
                                          self.directory / 'setup.exe', Path('makensis.exe'))
            compile_mock.assert_not_called()

    def test_existing_installer_cannot_be_overwritten(self):
        self.payload()
        output = self.directory / 'setup.exe'
        output.write_bytes(b'published installer')
        with self.assertRaises(FileExistsError):
            installer.compile_payload(self.directory, output, 'joyword', Path('makensis.exe'))
        self.assertEqual(output.read_bytes(), b'published installer')

    def test_missing_runtime_is_added_from_redist_and_fingerprinted(self):
        payload = self.directory / 'payload'
        (payload / 'licenses').mkdir(parents=True)
        redist = self.directory / '14.44.0/x64/Microsoft.VC143.CRT'
        redist.mkdir(parents=True)
        (redist / 'vcruntime140.dll').write_bytes(b'redistributable fixture')
        with patch.object(installer, 'runtime_directory', return_value=redist):
            result = installer.collect_runtime(payload, 'x86_64')
        self.assertEqual((payload / 'vcruntime140.dll').read_bytes(), b'redistributable fixture')
        self.assertEqual(result['sha256']['vcruntime140.dll'], installer.digest_file(redist / 'vcruntime140.dll'))

    def test_existing_payload_runtime_is_preserved_without_host_substitution(self):
        (self.directory / 'licenses').mkdir()
        (self.directory / 'vcruntime140.dll').write_bytes(b'original release bytes')
        with patch.object(installer, 'runtime_directory') as locate:
            result = installer.collect_runtime(self.directory, 'aarch64')
            locate.assert_not_called()
        self.assertEqual((self.directory / 'vcruntime140.dll').read_bytes(), b'original release bytes')
        self.assertEqual(result['origin'], 'original payload')


if __name__ == '__main__':
    unittest.main()
