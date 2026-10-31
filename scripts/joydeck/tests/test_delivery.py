"""Selected dependency graphs and relocated package paths must remain isolated."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import paths
import sbom
SPEC = importlib.util.spec_from_file_location('package_native', SCRIPTS / 'package-native.py')
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)

class DependencyGraph(unittest.TestCase):
    def metadata(self):
        return {'packages': [{'id': n, 'name': n, 'version': '1.0.0'} for n in
                             ['joydeck-cli', 'joydeck-shell', 'joydeck-model', 'joyword', 'gpui-kit']]}

    def test_cli_sbom_does_not_inherit_workspace_gui_nodes(self):
        reached, edges = sbom.selected_graph(self.metadata(), [
            '0joydeck-cli v1.0.0\n1joydeck-shell v1.0.0\n2joydeck-model v1.0.0\n'])
        self.assertEqual(reached, {'joydeck-cli', 'joydeck-shell', 'joydeck-model'})
        self.assertEqual(edges, {('joydeck-cli', 'joydeck-shell'), ('joydeck-shell', 'joydeck-model')})

    def test_joint_package_graph_is_union_of_selected_roots(self):
        reached, edges = sbom.selected_graph(self.metadata(), [
            '0joydeck-cli v1.0.0\n1joydeck-model v1.0.0\n',
            '0joyword v1.0.0\n1gpui-kit v1.0.0\n'])
        self.assertIn('gpui-kit', reached)
        self.assertNotIn(('joydeck-cli', 'gpui-kit'), edges)

    def test_malformed_tree_and_ambiguous_source_fail_closed(self):
        with self.assertRaises(ValueError):
            sbom.selected_graph(self.metadata(), ['0joydeck-cli v1.0.0\n2joydeck-model v1.0.0\n'])
        data = self.metadata()
        data['packages'].append({'id': 'other', 'name': 'joydeck-model', 'version': '1.0.0'})
        with self.assertRaises(ValueError):
            sbom.selected_graph(data, ['0joydeck-model v1.0.0\n'])

class PackagePaths(unittest.TestCase):
    def test_relocated_package_resolves_its_own_assets(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = (Path(tmp) / 'Chinese 中文 space').resolve()
            scripts = root / 'scripts'
            scripts.mkdir(parents=True)
            shutil.copy2(SCRIPTS / 'paths.py', scripts / 'paths.py')
            spec = importlib.util.spec_from_file_location('relocated_paths', scripts / 'paths.py')
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            self.assertFalse(module.WORKSPACE)
            self.assertEqual(module.ROOT, root)
            self.assertEqual(module.binary('joydeck-cli'), root / ('joydeck-cli.exe' if os.name == 'nt' else 'joydeck-cli'))
            self.assertEqual(module.FIXTURES, root / 'fixtures')
            self.assertEqual(module.native_helper('video'), root / 'joydeck-video-macos')

    def test_custom_target_directory_is_respected(self):
        paths.target_directory.cache_clear()
        self.addCleanup(paths.target_directory.cache_clear)
        with patch.dict(os.environ, {'CARGO_TARGET_DIR': 'target/custom'}):
            self.assertEqual(paths.binary('joydeck-cli'), paths.ROOT / 'target/custom/debug' /
                             ('joydeck-cli.exe' if os.name == 'nt' else 'joydeck-cli'))

    def test_packaging_uses_actual_cargo_executable_not_guessed_path(self):
        value = {'reason': 'compiler-artifact', 'target': {'name': 'joydeck-cli'},
                 'executable': '/custom/build-target/joydeck-cli'}
        completed = type('Result', (), {'stdout': json.dumps(value) + '\n'})()
        with patch.object(package.subprocess, 'run', return_value=completed), patch.object(package.subprocess, 'check_output', return_value='host: aarch64-apple-darwin\n'):
            self.assertEqual(package.build_binary('joydeck-cli', 'release'), Path(value['executable']))
        completed.stdout = ''
        with patch.object(package.subprocess, 'run', return_value=completed), patch.object(package.subprocess, 'check_output', return_value='host: aarch64-apple-darwin\n'):
            with self.assertRaises(ValueError):
                package.build_binary('joydeck-cli', 'release')

if __name__ == '__main__':
    unittest.main()
