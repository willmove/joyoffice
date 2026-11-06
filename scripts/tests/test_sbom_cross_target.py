"""Cross-target inventories retain host build dependencies without adding other targets."""
import json
import unittest
from unittest import mock

from test_package_archive import package
from test_sbom_local_sources import sbom


class CrossTargetInventory(unittest.TestCase):
    def setUp(self):
        self.packages = [dict(name=name, version='1.0.0', id=name)
                         for name in ('joydeck', 'joydeck-cli', 'build-tool', 'core_detect', 'unselected')]

    def output(self, command, **kwargs):
        if command[1] == 'metadata':
            packages = self.packages
            # Cargo's target-only metadata excludes this x86 host dependency.
            if '--filter-platform' in command:
                packages = [p for p in packages if p['name'] != 'core_detect']
            return json.dumps({'packages': packages})
        self.assertEqual(command[command.index('--target') + 1], 'aarch64-apple-darwin')
        self.assertEqual(command[command.index('--edges') + 1], 'normal,build')
        if command[command.index('--prefix') + 1] == 'none':
            return '\n'.join(p['name'] + ' v1.0.0' for p in self.packages[:-1])
        root = command[command.index('-p') + 1]
        return f'0{root} v1.0.0\n1build-tool v1.0.0\n2core_detect v1.0.0\n'

    def test_spdx_retains_host_dependency_and_excludes_unselected_package(self):
        with mock.patch.object(sbom.subprocess, 'check_output', side_effect=self.output):
            _, _, reached, edges, roots = sbom.graph('aarch64-apple-darwin')
        self.assertEqual(reached, {'joydeck', 'joydeck-cli', 'build-tool', 'core_detect'})
        self.assertIn(('build-tool', 'core_detect'), edges)
        self.assertEqual(roots, {'joydeck', 'joydeck-cli'})

    def test_native_package_inventory_retains_host_dependency(self):
        with mock.patch.object(package.subprocess, 'check_output', side_effect=self.output):
            packages = package.sbom('aarch64-apple-darwin')
        self.assertEqual({p['id'] for p in packages}, {'joydeck', 'joydeck-cli', 'build-tool', 'core_detect'})

    def test_missing_and_ambiguous_identities_still_fail_closed(self):
        for packages, message in [([], 'Missing'),
                                  ([dict(name='dep', version='1', id='one'),
                                    dict(name='dep', version='1', id='two')], 'Ambiguous')]:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                sbom.selected_graph({'packages': packages}, ['0dep v1'])
