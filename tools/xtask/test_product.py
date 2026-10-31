"""Regression tests for product isolation and release routing, without building."""
import importlib.util
from pathlib import Path
import tempfile
import subprocess
import sys
import json
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('product', Path(__file__).with_name('product.py'))
product = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(product)

def graph():
    names = ['joydeck', 'joydeck-cli', 'joydeck-model', 'joydeck-shell', 'joyword',
             'joyword-cli', 'joyword-model', 'joyoffice-opc', 'joyoffice-cfb', 'joyoffice-omml', 'joyoffice-xtask',
             'joysheet', 'joysheet-cli', 'joysheet-model', 'joysheet-engine', 'joysheet-ui-gpui', 'joysheet-platform']
    return {'workspace_members': names,
            'packages': [{'id': n, 'name': n} for n in names],
            'resolve': {'nodes': [{'id': n, 'deps': [{'pkg': d} for d in {
                'joydeck-shell': ['joyoffice-opc'], 'joyoffice-opc': ['joyoffice-cfb']}.get(n, [])]}
                for n in names]}}

class ProductRouting(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        for name, version in [('joyword', '0.2.1'), ('joydeck', '0.1.3'), ('joysheet', '0.1.4')]:
            directory = self.root / 'apps' / name
            directory.mkdir(parents=True)
            (directory / 'Cargo.toml').write_text('[package]\nname = "' + name + '"\nversion = "' + version + '"\n', encoding='utf-8')
            cli = self.root / 'apps' / (name + '-cli')
            cli.mkdir()
            (cli / 'Cargo.toml').write_text('[package]\nname = "' + name + '-cli"\nversion = "' + version + '"\n', encoding='utf-8')
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nversion = "9.9.9"\n', encoding='utf-8')

    def test_versions_are_product_scoped(self):
        self.assertEqual(product.manifest_version('joydeck', self.root), '0.1.3')
        self.assertEqual(product.manifest_version('joyword', self.root), '0.2.1')

    def test_legacy_tags_route_only_to_word(self):
        self.assertEqual(product.release_target('v0.2.1', root=self.root),
                         {'product': 'joyword', 'version': '0.2.1', 'tag': 'v0.2.1'})
        with self.assertRaises(ValueError):
            product.release_target('v0.1.3', root=self.root)

    def test_product_tag_cannot_release_other_product(self):
        self.assertEqual(product.release_target('joydeck-v0.1.3', root=self.root)['product'], 'joydeck')
        with self.assertRaises(ValueError):
            product.release_target('joydeck-v0.2.1', root=self.root)
        with self.assertRaises(ValueError):
            product.release_target('joydeck-v0.1.3', product='joyword', root=self.root)
        with self.assertRaises(ValueError):
            product.release_target('joysheet-v0.1.0', root=self.root)

    def test_cli_version_must_match_application(self):
        (self.root / 'apps/joydeck-cli/Cargo.toml').write_text('[package]\nversion = "0.1.2"\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'CLI version'):
            product.release_target('joydeck-v0.1.3', root=self.root)

    def test_manual_release_requires_matching_product_version(self):
        value = product.release_target(product='joydeck', version='0.1.3', root=self.root)
        self.assertEqual(value['tag'], 'joydeck-v0.1.3')
        with self.assertRaises(ValueError):
            product.release_target(product='joydeck', version='9.9.9', root=self.root)

    def test_tests_select_owned_and_consumed_shared_packages(self):
        self.assertEqual(product.product_packages(graph(), 'joydeck', headless=True),
                         ['joydeck-cli', 'joydeck-model', 'joydeck-shell', 'joyoffice-cfb', 'joyoffice-opc'])
        with patch.object(product, 'metadata', return_value=graph()), patch.object(product, 'run') as run:
            product.main(['test', 'joydeck', '--headless'])
        command = run.call_args.args[0]
        self.assertNotIn('--workspace', command)
        self.assertNotIn('joydeck', command)
        self.assertNotIn('joyword-model', command)
        self.assertIn('joydeck-model', command)
        self.assertIn('joyoffice-opc', command)

    def test_cross_product_dependency_is_rejected(self):
        data = graph()
        data['resolve']['nodes'][7]['deps'].append({'pkg': 'joyword-model'})
        with self.assertRaisesRegex(ValueError, 'another product'):
            product.product_packages(data, 'joydeck')

    def test_cargo_metadata_decodes_utf8_under_windows_ansi_locale(self):
        metadata = {'workspace_members': [], 'packages': [], 'unicode': '中文文稿😀'}
        payload = json.dumps(metadata, ensure_ascii=False).encode('utf-8')
        real_output = subprocess.check_output
        def cargo_output(command, **kwargs):
            self.assertEqual(command[:2], ['cargo', 'metadata'])
            return real_output([sys.executable, '-c',
                                'import sys; sys.stdout.buffer.write(' + repr(payload) + ')'], **kwargs)
        # Force the exact non-UTF8 text default used on the failing Windows
        # runner. Without explicit encoding, this corrupts or rejects metadata.
        with patch.object(product.subprocess, 'check_output', side_effect=cargo_output), \
                patch.object(subprocess, '_text_encoding', return_value='cp1252'):
            self.assertEqual(product.metadata(), metadata)

    def test_build_selects_only_application_entrypoints(self):
        with patch.object(product, 'metadata', return_value=graph()), patch.object(product, 'run') as run:
            product.main(['build', 'joydeck', '--release'])
        self.assertEqual(run.call_args.args[0],
                         ['cargo', 'build', '-p', 'joydeck', '-p', 'joydeck-cli', '--locked', '--release'])

    def test_sheet_release_uses_its_own_version_and_tag(self):
        self.assertEqual(product.release_target('joysheet-v0.1.4', root=self.root),
                         {'product': 'joysheet', 'version': '0.1.4', 'tag': 'joysheet-v0.1.4'})
        with self.assertRaisesRegex(ValueError, 'does not belong'):
            product.release_target('joysheet-v0.1.4', product='joydeck', root=self.root)

    def test_sheet_headless_selects_core_and_cli_without_ui_adapter(self):
        chosen = product.product_packages(graph(), 'joysheet', headless=True)
        self.assertEqual(chosen, ['joysheet-cli', 'joysheet-engine', 'joysheet-model'])
        with patch.object(product, 'metadata', return_value=graph()), patch.object(product, 'run') as run:
            product.main(['test', 'joysheet', '--headless'])
        command = run.call_args.args[0]
        self.assertIn('joysheet-model', command)
        self.assertNotIn('joysheet-platform', command)
        self.assertNotIn('joysheet-ui-gpui', command)

    def test_headless_cannot_hide_a_transitive_gui_dependency(self):
        data = graph()
        data['packages'].append({'id': 'gpui-registry', 'name': 'gpui-pre'})
        data['resolve']['nodes'].append({'id': 'gpui-registry', 'deps': []})
        node = next(n for n in data['resolve']['nodes'] if n['id'] == 'joysheet-model')
        node['deps'].append({'pkg': 'gpui-registry'})
        with self.assertRaisesRegex(ValueError, 'GPUI'):
            product.product_packages(data, 'joysheet', headless=True)

    def test_sheet_validation_enables_automation_explicitly(self):
        with patch.object(product, 'metadata', return_value=graph()), patch.object(product, 'run') as run:
            product.main(['test', 'joysheet', '--validation'])
        self.assertIn('joysheet/automation', run.call_args.args[0])
        with patch.object(product, 'metadata', return_value=graph()), patch.object(product, 'run') as run:
            product.main(['build', 'joysheet'])
        self.assertNotIn('--features', run.call_args.args[0])

if __name__ == '__main__':
    unittest.main()
