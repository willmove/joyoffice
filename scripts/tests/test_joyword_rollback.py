"""Test rollback failure handling without claiming Windows desktop acceptance."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch


SPEC = importlib.util.spec_from_file_location(
    "joyword_rollback", Path(__file__).resolve().parents[1] / "joyword/verify-r8-rollback.py")
rollback = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rollback)


class Rollback(unittest.TestCase):
    def test_validation_failure_restores_previous(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            install, staged, previous, retired = [root / name for name in
                                                 ("install", "stage", "previous", "retired")]
            install.mkdir()
            staged.mkdir()
            (install / "binary").write_bytes(b"old")
            (staged / "binary").write_bytes(b"new")
            with self.assertRaisesRegex(ValueError, "candidate failed"):
                with rollback.candidate_in_place(install, staged, previous, retired):
                    self.assertEqual((install / "binary").read_bytes(), b"new")
                    rollback.require(False, "candidate failed")
            self.assertEqual((install / "binary").read_bytes(), b"old")
            self.assertEqual((retired / "binary").read_bytes(), b"new")
            self.assertFalse(previous.exists())

    def test_missing_stage_restores_previous(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            install = root / "install"
            install.mkdir()
            (install / "binary").write_bytes(b"old")
            with self.assertRaises(FileNotFoundError):
                with rollback.candidate_in_place(install, root / "missing", root / "previous",
                                                 root / "retired"):
                    self.fail("Missing stage was promoted")
            self.assertEqual((install / "binary").read_bytes(), b"old")
            self.assertFalse((root / "previous").exists())
            self.assertFalse((root / "retired").exists())

    def test_authentication_failure_is_checked_and_stops_candidate(self):
        process = Mock()
        process.poll.return_value = None
        with patch.object(rollback.socket, "socket"), \
                patch.object(rollback, "start", return_value=process), \
                patch.object(rollback, "stop") as stop, \
                patch.object(rollback, "request", side_effect=[
                    {"ok": True, "result": {}}, {"error": "accepted"}]) as request:
            with self.assertRaisesRegex(ValueError, "did not reject a missing key"):
                rollback.verify_control(Path("candidate"), {}, Path("source.docx"))
            self.assertEqual(request.call_count, 2)
            stop.assert_called_once_with(process)

    def test_optimized_python_keeps_authentication_check(self):
        result = subprocess.run(
            [sys.executable, "-O", str(Path(__file__).resolve()),
             "Rollback.test_authentication_failure_is_checked_and_stops_candidate"],
            capture_output=True, text=True, timeout=30, check=False)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    @unittest.skipIf(sys.platform == "win32", "Non-Windows refusal")
    def test_full_run_refuses_non_windows_before_creating_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "output"
            with self.assertRaisesRegex(ValueError, "requires Windows"):
                rollback.verify(root / "old", root / "new", root / "fixtures", output)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
