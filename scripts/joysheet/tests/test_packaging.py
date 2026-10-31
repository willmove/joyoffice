"""Exercise package boundaries and Cargo artifact selection without rebuilding."""
import importlib.util
import json
from pathlib import Path
import sys
import tomllib
import unittest
import tempfile
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import project
SPEC = importlib.util.spec_from_file_location("sheet_package", SCRIPTS / "package.py")
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class Packaging(unittest.TestCase):
    def license_inventory(self, root):
        source = root / 'registry/fake-native-1.0.0'
        source.mkdir(parents=True)
        (source / 'Cargo.toml').write_text('[package]\nname="fake-native"\n', encoding='utf-8')
        (root / 'Cargo.lock').write_text('version = 4\n', encoding='utf-8')
        item = {'id': 'native', 'name': 'fake-native', 'version': '1.0.0', 'license': 'MIT',
                'source': 'registry+https://github.com/rust-lang/crates.io-index',
                'manifest_path': str(source / 'Cargo.toml')}
        return ({'workspace_members': []}, {'native': item}, {'native'}, set(), set())

    def test_declared_license_without_text_stops_distribution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            graph = self.license_inventory(root)
            with patch.object(package.sbom, 'ROOT', root), patch.object(package.sbom, 'LICENSES', root / 'legacy'), \
                    patch.object(package.sbom, 'graph', return_value=graph):
                with self.assertRaisesRegex(ValueError, 'fake-native@1.0.0'):
                    package.sbom.generate(root / 'SBOM.json', 'host', roots=project.APPLICATIONS, require_license_texts=True)
            self.assertFalse((root / 'SBOM.json').exists())

    def test_exact_version_supplement_is_copied_verbatim(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            graph = self.license_inventory(root)
            supplement = root / 'docs/joysheet/third-party/fake-native/1.0.0'
            supplement.mkdir(parents=True)
            text = b'MIT license text from the recorded upstream source.\r\n'
            (supplement / 'LICENSE.txt').write_bytes(text)
            with patch.object(package.sbom, 'ROOT', root), patch.object(package.sbom, 'LICENSES', root / 'legacy'), \
                    patch.object(package.sbom, 'graph', return_value=graph):
                result = package.sbom.generate(root / 'SBOM.json', 'host', roots=project.APPLICATIONS, require_license_texts=True)
            self.assertTrue(result['licenseTextsVerified'])
            self.assertEqual((root / 'dependency-licenses/fake-native-1.0.0/LICENSE.txt').read_bytes(), text)

    def test_wrong_version_and_provenance_alone_do_not_satisfy_text_requirement(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            graph = self.license_inventory(root)
            base = root / 'docs/joysheet/third-party/fake-native'
            (base / '0.9.0').mkdir(parents=True)
            (base / '0.9.0/LICENSE.txt').write_text('Old license', encoding='utf-8')
            (base / '1.0.0').mkdir()
            (base / '1.0.0/SOURCE.json').write_text('{}', encoding='utf-8')
            with patch.object(package.sbom, 'ROOT', root), patch.object(package.sbom, 'LICENSES', root / 'legacy'), \
                    patch.object(package.sbom, 'graph', return_value=graph):
                with self.assertRaisesRegex(ValueError, 'Missing dependency license texts'):
                    package.sbom.generate(root / 'SBOM.json', 'host', roots=project.APPLICATIONS, require_license_texts=True)

    def test_standalone_workspace_keeps_consumed_shared_crates_and_features(self):
        source = '''[workspace]
members = ["apps/joyword", "apps/joysheet", "crates/joyoffice-opc"]
default-members = ["apps/joyword"]
resolver = "3"
[workspace.dependencies]
joyword-model = { path = "crates/joyword-model" }
joysheet-core = { path = "crates/joysheet-core" }
joyoffice-opc = { path = "crates/joyoffice-opc" }
serde_json = { version = "=1.0.154", features = ["float_roundtrip"] }
'''
        members = ["apps/joysheet", "crates/joysheet-core", "crates/joyoffice-opc"]
        actual = tomllib.loads(package.filtered_manifest(source, members))["workspace"]
        self.assertEqual(set(actual["members"]), set(members))
        self.assertNotIn("joyword-model", actual["dependencies"])
        self.assertIn("joyoffice-opc", actual["dependencies"])
        self.assertEqual(actual["dependencies"]["serde_json"]["features"], ["float_roundtrip"])

    def test_native_build_uses_cargo_reported_paths_and_explicit_host(self):
        outputs = {"joysheet": "/custom-cache/host/debug/joysheet.exe",
                   "joysheet-cli": "/custom-cache/host/debug/joysheet-cli.exe"}
        payload = "\n".join(json.dumps({"reason": "compiler-artifact", "target": {"name": name}, "executable": path})
                            for name, path in outputs.items())
        with patch.object(project, "host_target", return_value="x86_64-pc-windows-msvc"), \
                patch.object(project.subprocess, "check_output", return_value=payload) as cargo:
            result = project.build(validation=True)
        self.assertEqual({k: str(v).replace("\\", "/") for k, v in result.items()}, outputs)
        command = cargo.call_args.args[0]
        self.assertIn("x86_64-pc-windows-msvc", command)
        self.assertIn("joysheet/automation", command)

    def test_build_rejects_missing_cli_artifact(self):
        payload = json.dumps({"reason": "compiler-artifact", "target": {"name": "joysheet"}, "executable": "/cache/joysheet"})
        with patch.object(project, "host_target", return_value="host"), \
                patch.object(project.subprocess, "check_output", return_value=payload):
            with self.assertRaisesRegex(ValueError, "both Sheet executables"):
                project.build()

    def test_inventory_metadata_uses_the_same_features_as_the_selected_tree(self):
        names = [('joysheet', '0.1.0'), ('joysheet-cli', '0.1.0'), ('feature-only', '1.0.0')]
        data = {'packages': [{'id': name, 'name': name, 'version': version} for name, version in names]}
        replies = [json.dumps(data), '0joysheet v0.1.0\n1feature-only v1.0.0\n', '0joysheet-cli v0.1.0\n']
        with patch.object(package.sbom.subprocess, 'check_output', side_effect=replies) as cargo:
            _, _, reached, _, _ = package.sbom.graph('host', roots=project.APPLICATIONS,
                                                    features=('joysheet/automation',))
        self.assertIn('feature-only', reached)
        metadata_command = cargo.call_args_list[0].args[0]
        self.assertIn('joysheet/automation', metadata_command)
        self.assertNotIn('--features', cargo.call_args_list[2].args[0])


if __name__ == "__main__":
    unittest.main()
