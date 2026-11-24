"""Exercise package boundaries and Cargo artifact selection without rebuilding."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tomllib
import unittest
import tempfile
import zipfile
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
            self.assertEqual(result['missingLicenseTexts'], [])
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

    def test_windows_installer_uses_the_final_portable_payload(self):
        import native

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            docs = root / "docs/joysheet"
            docs.mkdir(parents=True)
            (root / "scripts/joysheet").mkdir(parents=True)
            (root / "LICENSE").touch()
            for name in ("LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md"):
                (docs / name).touch()
            artifacts = {name: root / (name + ".exe") for name in ("joysheet", "joysheet-cli")}
            for path in artifacts.values():
                path.write_bytes(b"fixture binary")
            destination = root / "payload"
            runtime = destination / "vcruntime140.dll"
            manifest = destination / "PAYLOAD-SHA256.json"
            phases = []

            def run(command):
                if "-PrepareOnly" in command:
                    self.assertFalse(manifest.exists())
                    runtime.write_bytes(b"fixture runtime")
                    phases.append("prepare")
                    return
                names = json.loads(manifest.read_text(encoding="utf-8"))
                self.assertEqual(names[runtime.name], hashlib.sha256(runtime.read_bytes()).hexdigest())
                with zipfile.ZipFile(root / "payload.zip") as portable:
                    self.assertEqual(portable.read("payload/PAYLOAD-SHA256.json"), manifest.read_bytes())
                    self.assertEqual(portable.read("payload/vcruntime140.dll"), runtime.read_bytes())
                phases.append("compile")

            with (
                patch.object(package, "ROOT", root),
                patch.object(package, "INTEGRATION_DOCS", ()),
                patch.object(package, "manifest_version", return_value="0.2.0"),
                patch.object(package, "host_target", return_value="x86_64-pc-windows-msvc"),
                patch.object(package, "build", return_value=artifacts),
                patch.object(package, "source_snapshot", return_value={}),
                patch.object(package, "source_archive"),
                patch.object(package.sbom, "generate", return_value={"lockSha256": "fixture"}),
                patch.object(package.platform, "system", return_value="Windows"),
                patch.object(native, "run", side_effect=run),
            ):
                self.assertEqual(package.package("release", False, destination), destination.resolve())
            self.assertEqual(phases, ["prepare", "compile"])

    def test_linux_bundle_uses_absolute_system_icon_and_relocatable_appdir(self):
        import native

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / 'payload'
            directory.mkdir()
            (directory / 'BUILD.json').write_text('{"target":"x86_64-unknown-linux-gnu"}')
            for name in ('joysheet', 'joysheet-cli', 'LICENSE-JOYOFFICE'):
                (directory / name).write_bytes(b'fixture')
            (directory / 'dependency-licenses').mkdir()
            with patch.object(native.platform, 'system', return_value='Linux'), patch.object(native, 'run') as run_cmd:
                native.bundle(directory, '0.3.0')
            installed = directory / 'debian/usr/share/applications/joysheet.desktop'
            self.assertIn('Icon=/usr/share/icons/hicolor/scalable/apps/joysheet.svg\n', installed.read_text())
            control = (directory / 'debian/DEBIAN/control').read_text(encoding='utf-8')
            self.assertIn('libc6 (>= 2.28)', control)
            self.assertNotIn('libssl3t64', control)
            self.assertNotIn('2.39', control)
            dpkg_calls = [call.args[0] for call in run_cmd.call_args_list if call.args[0][0] == 'dpkg-deb']
            self.assertEqual(len(dpkg_calls), 1)
            self.assertIn('-Zxz', dpkg_calls[0])
            appdir = directory / 'Joysheet.AppDir/joysheet.desktop'
            self.assertIn('Icon=joysheet\n', appdir.read_text())
            self.assertNotIn(str(directory), installed.read_text() + appdir.read_text())

    def test_source_snapshot_keeps_patches_outside_host_dependency_graph(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            patch_manifest = root / "third-party/gpui-pre-linux/Cargo.toml"
            patch_manifest.parent.mkdir(parents=True)
            patch_manifest.write_bytes(b"local Linux patch manifest")
            (root / "Cargo.toml").write_text(
                '[patch.crates-io]\ngpui-pre-linux = { path = "third-party/gpui-pre-linux" }\n',
                encoding="utf-8",
            )
            (root / "LICENSE").touch()
            with (
                patch.object(package, "ROOT", root),
                patch.object(package, "INTEGRATION_DOCS", ()),
                patch.object(package.sbom, "snapshot", return_value={}),
            ):
                snapshot = package.source_snapshot("x86_64-pc-windows-msvc", ())
            self.assertEqual(snapshot["third-party/gpui-pre-linux/Cargo.toml"],
                             hashlib.sha256(patch_manifest.read_bytes()).hexdigest())

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
