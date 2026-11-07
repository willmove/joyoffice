"""Exercise the headless rollback entry point with isolated executable stand-ins."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "headless_rollback", Path(__file__).resolve().parents[1] / "joyword/verify-r8-headless-rollback.py")
rollback = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rollback)


@unittest.skipUnless(sys.platform.startswith("linux"), "Linux executable exercise")
class HeadlessRollback(unittest.TestCase):
    def setup_inputs(self, root, mutate=False):
        fixtures = root / "fixtures"
        (fixtures / "rich").mkdir(parents=True)
        (fixtures / "source.docx").write_bytes(b"original")
        (fixtures / "snapshot.docx").write_bytes(b"snapshot")
        (fixtures / "rich" / "recovery-a.jwr").write_bytes(b"journal")
        code = f'''#!{sys.executable}
import json,pathlib,sys
if sys.argv[1] == "inspect":
 p=pathlib.Path(sys.argv[2])
 if not p.is_file(): sys.exit(2)
 print(json.dumps({{"text":p.read_text()}}))
elif sys.argv[1] == "recover":
 pathlib.Path(sys.argv[4]).write_bytes(b"snapshot")
 print("{{}}")
'''
        old, new = root / "old-cli", root / "new-cli"
        old.write_text(code)
        if mutate:
            code += '\npathlib.Path("../源 文档.docx").write_bytes(b"changed")\n'
        new.write_text(code + '\n# candidate build\n')
        for path in (old, new):
            path.chmod(0o700)
        return old, new, fixtures

    def test_real_subprocesses_and_rollback_report_scope(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            old, new, fixtures = self.setup_inputs(root)
            out = root / "中文 空格"
            report = rollback.verify(old, new, fixtures, out)
            self.assertTrue(report["candidate_process_failure_restored_previous"])
            self.assertTrue(report["exact_v3_snapshot"])
            self.assertFalse(report["gui_verified"])
            self.assertFalse(report["release_package_verified"])
            self.assertEqual(rollback.digest(out / "install/joyword-cli"), rollback.digest(old))
            self.assertEqual(json.loads((out / "verification.json").read_text()), report)
            with self.assertRaisesRegex(ValueError, "must be new"):
                rollback.verify(old, new, fixtures, out)

    def test_original_mutation_cannot_be_reported_as_success(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            old, new, fixtures = self.setup_inputs(root, mutate=True)
            out = root / "out"
            with self.assertRaisesRegex(ValueError, "report differs|file changed|source report differs"):
                rollback.verify(old, new, fixtures, out)
            self.assertFalse((out / "verification.json").exists())
            self.assertEqual(rollback.digest(out / "install/joyword-cli"), rollback.digest(old))

    def test_wrong_platform_and_identical_inputs_refuse_before_output(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            old, new, fixtures = self.setup_inputs(root)
            out = root / "out"
            with patch.object(rollback.sys, "platform", "win32"):
                with self.assertRaisesRegex(ValueError, "requires Linux"):
                    rollback.verify(old, new, fixtures, out)
            with self.assertRaisesRegex(ValueError, "must differ"):
                rollback.verify(old, old, fixtures, out)
            self.assertFalse(out.exists())


if __name__ == "__main__":
    unittest.main()
