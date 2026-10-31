"""Regression tests for product isolation and release routing, without building."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('product', Path(__file__).with_name('product.py'))
product = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(product)

def graph():
    names = ['joydeck', 'joydeck-cli', 'joydeck-model', 'joydeck-shell', 'joyword',
             'joyword-cli', 'joyword-model', 'joyoffice-opc', 'joyoffice-cfb', 'joyoffice-omml', 'joyoffice-xtask']
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
        for name, version in [('joyword', '0.2.1'), ('joydeck', '0.1.3')]:
            directory = self.root / 'apps' / name
            directory.mkdir(parents=True)
            (directory / 'Cargo.toml').write_text('[package]\nname = "' + name + '"\nversion = "' + version + '"\n')
        (self.root / 'Cargo.toml').write_text('[workspace.package]\nversion = "9.9.9"\n')

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

    def test_build_selects_only_application_entrypoints(self):
        with patch.object(product, 'metadata', return_value=graph()), patch.object(product, 'run') as run:
            product.main(['build', 'joydeck', '--release'])
        self.assertEqual(run.call_args.args[0],
                         ['cargo', 'build', '-p', 'joydeck', '-p', 'joydeck-cli', '--locked', '--release'])

if __name__ == '__main__':
    unittest.main()
