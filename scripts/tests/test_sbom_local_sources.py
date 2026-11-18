"""A local patched dependency must be part of the product source fingerprint."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1] / 'joydeck'
sys.path.insert(0, str(SCRIPTS))
try:
    spec = importlib.util.spec_from_file_location('local_source_sbom', SCRIPTS / 'sbom.py')
    sbom = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(sbom)
finally:
    sys.path.pop(0)


class LocalDependencySources(unittest.TestCase):
    def test_shared_ui_fingerprints_every_embedded_brand(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            ui = root / 'crates/joyoffice-ui'
            ui.mkdir(parents=True)
            (ui / 'Cargo.toml').write_text('[package]\nname="joyoffice-ui"\n')
            assets = root / 'assets/brand'
            assets.mkdir(parents=True)
            for product in ('joyword', 'joydeck', 'joysheet'):
                for extension in ('svg', 'ico'):
                    (assets / f'{product}.{extension}').write_bytes(b'artwork')
            packages = {'ui': {'name': 'joyoffice-ui', 'source': None,
                               'manifest_path': str(ui / 'Cargo.toml')}}
            with mock.patch.object(sbom, 'ROOT', root):
                before = sbom.source_hashes({'workspace_members': []}, packages, {'ui'})
                (assets / 'joydeck.ico').write_bytes(b'changed artwork')
                after = sbom.source_hashes({'workspace_members': []}, packages, {'ui'})
            for asset in assets.iterdir():
                self.assertIn(asset.relative_to(root).as_posix(), before)
            self.assertNotEqual(before['assets/brand/joydeck.ico'], after['assets/brand/joydeck.ico'])

    def test_excluded_local_dependency_changes_the_product_fingerprint(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            dependency = root / 'third-party/adapter'
            (dependency / 'src').mkdir(parents=True)
            (dependency / 'Cargo.toml').write_text('[package]\nname="adapter"\n')
            source = dependency / 'src/lib.rs'
            source.write_text('pub fn retained_provider() {}\n')
            packages = {'adapter': {'name': 'adapter', 'source': None,
                                    'manifest_path': str(dependency / 'Cargo.toml')}}
            with mock.patch.object(sbom, 'ROOT', root):
                before = sbom.source_hashes({'workspace_members': []}, packages, {'adapter'})
                source.write_text('pub fn vanished_provider() {}\n')
                after = sbom.source_hashes({'workspace_members': []}, packages, {'adapter'})
            key = 'third-party/adapter/src/lib.rs'
            self.assertIn(key, before)
            self.assertNotEqual(before[key], after[key])
            with self.assertRaises(ValueError):
                sbom.require_unchanged_sources(before, after)
