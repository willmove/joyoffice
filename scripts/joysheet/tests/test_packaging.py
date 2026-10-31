"""Exercise package boundaries and Cargo artifact selection without rebuilding."""
import importlib.util
import json
from pathlib import Path
import sys
import tomllib
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
import project
SPEC = importlib.util.spec_from_file_location("sheet_package", SCRIPTS / "package.py")
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


class Packaging(unittest.TestCase):
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
