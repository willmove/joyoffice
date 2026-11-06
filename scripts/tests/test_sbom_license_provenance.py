"""Recorded license hashes must be checked before generating an inventory."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from test_sbom_local_sources import sbom


class LicenseProvenance(unittest.TestCase):
    def inventory(self, root, body=b'Original notice\r\n'):
        source = root / 'registry/demo'
        source.mkdir(parents=True)
        (source / 'Cargo.toml').write_text('[package]\nname="demo"\n')
        (root / 'Cargo.lock').write_text('version = 4\n')
        item = {'name': 'demo', 'version': '1.0.0', 'source': 'registry+test',
                'license': 'MIT', 'manifest_path': str(source / 'Cargo.toml')}
        folder = root / 'licenses/dependency-notices/demo-1.0.0'
        folder.mkdir(parents=True)
        (folder / 'LICENSE').write_bytes(body)
        records = [{'file': 'LICENSE', 'url': 'https://example.org/pinned/LICENSE',
                    'sha256': hashlib.sha256(body).hexdigest()}]
        (folder / 'SOURCES.json').write_text(json.dumps(records))
        graph = ({'workspace_members': []}, {'demo': item}, {'demo'}, set(), set())
        return folder, graph

    def generate(self, root, graph, strict=True):
        with mock.patch.object(sbom, 'ROOT', root), mock.patch.object(sbom, 'LICENSES', root / 'licenses'), \
                mock.patch.object(sbom, 'graph', return_value=graph):
            return sbom.generate(root / 'SBOM.json', 'host', require_license_texts=strict)

    def test_tampered_supplement_stops_both_development_and_strict_generation(self):
        for strict in [False, True]:
            with self.subTest(strict=strict), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                folder, graph = self.inventory(root)
                (folder / 'LICENSE').write_bytes(b'Changed notice')
                with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                    self.generate(root, graph, strict)
                self.assertFalse((root / 'SBOM.json').exists())

    def test_valid_supplement_is_copied_verbatim(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            folder, graph = self.inventory(root)
            result = self.generate(root, graph)
            self.assertEqual(result['missingLicenseTexts'], [])
            self.assertEqual((root / 'dependency-licenses/demo-1.0.0/LICENSE').read_bytes(),
                             (folder / 'LICENSE').read_bytes())

    def test_invalid_manifests_missing_files_and_unrecorded_additions_are_rejected(self):
        for case in ['invalid-json', 'not-list', 'empty', 'duplicate', 'traversal', 'bad-hash', 'missing', 'extra']:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                folder, graph = self.inventory(root)
                manifest = folder / 'SOURCES.json'
                records = json.loads(manifest.read_text())
                if case == 'invalid-json':
                    manifest.write_text('{')
                elif case == 'missing':
                    (folder / 'LICENSE').unlink()
                elif case == 'extra':
                    (folder / 'LICENSE-extra').write_text('unrecorded')
                else:
                    if case == 'not-list': records = {}
                    elif case == 'empty': records = []
                    elif case == 'duplicate': records *= 2
                    elif case == 'traversal': records[0]['file'] = '../LICENSE'
                    else: records[0]['sha256'] = 'not a digest'
                    manifest.write_text(json.dumps(records))
                with self.assertRaises(ValueError):
                    self.generate(root, graph)
                self.assertFalse((root / 'SBOM.json').exists())

    def test_symlink_notice_is_rejected_even_if_hash_matches(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            folder, graph = self.inventory(root)
            notice = folder / 'LICENSE'
            outside = root / 'outside'
            outside.write_bytes(notice.read_bytes())
            notice.unlink()
            try:
                notice.symlink_to(outside)
            except OSError as error:
                self.skipTest(str(error))
            with self.assertRaisesRegex(ValueError, 'Symlink'):
                self.generate(root, graph)

    def test_changed_bytes_during_copy_are_not_published_as_verified(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            folder, graph = self.inventory(root)
            original_copy = sbom.shutil.copy2
            def copy(source, destination):
                result = original_copy(source, destination)
                if source == folder / 'LICENSE':
                    destination.write_bytes(b'changed during copy')
                return result
            with mock.patch.object(sbom.shutil, 'copy2', side_effect=copy):
                with self.assertRaisesRegex(ValueError, 'hash mismatch after copy'):
                    self.generate(root, graph)
            self.assertFalse((root / 'SBOM.json').exists())

    def test_committed_deck_supplements_match_their_recorded_hashes(self):
        folders = sorted((sbom.LICENSES / 'dependency-notices').glob('*/SOURCES.json'))
        self.assertTrue(folders)
        for manifest in folders:
            with self.subTest(package=manifest.parent.name):
                hashes = sbom.verified_supplement_files(manifest.parent)
                self.assertIn(manifest, hashes)
