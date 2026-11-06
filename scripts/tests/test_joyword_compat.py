"""A matching body must not hide a schema-1 compatibility regression."""
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "joyword_compat", Path(__file__).resolve().parents[1] / "joyword/verify-r8-compat.py")
compat = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(compat)


class CompatibilityGate(unittest.TestCase):
    def setUp(self):
        self.baseline = {"schema_version": 1, "text": "正文😀", "omitted_objects": 2,
                         "preserved_fragments": 1, "read_only": True}

    def verify_corpus(self, current):
        # Exercise the real corpus loop up to its comparison, with CLI responses
        # substituted so these gate tests run without a platform-specific binary.
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixtures = root / "tests/fixtures/docx"
            fixtures.mkdir(parents=True)
            (fixtures / "sample.docx").write_bytes(b"unchanged fixture")
            metadata = {k: v for k, v in current.items() if k != "text"}
            response = subprocess.CompletedProcess([], 0, current["text"].encode(),
                                                   json.dumps(metadata).encode())
            with patch.object(compat, "ROOT", root), \
                    patch.object(compat, "inspect", side_effect=[current, self.baseline]), \
                    patch.object(compat, "run", return_value=response):
                compat.verify("candidate", fixtures, root, "baseline")

    def test_matching_body_does_not_hide_diagnostic_changes(self):
        for key, value in [("omitted_objects", 3), ("preserved_fragments", 0),
                           ("read_only", False)]:
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, key):
                self.verify_corpus({**self.baseline, key: value})

    def test_removed_key_is_a_reported_mismatch(self):
        current = dict(self.baseline)
        del current["read_only"]
        with self.assertRaisesRegex(ValueError, "legacy report differs.*read_only"):
            self.verify_corpus(current)

    def test_added_key_is_not_silently_ignored(self):
        with self.assertRaisesRegex(ValueError, "legacy report differs.*new_diagnostic"):
            self.verify_corpus({**self.baseline, "new_diagnostic": None})

    def test_equal_reports_ignore_json_key_order(self):
        current = dict(reversed(list(self.baseline.items())))
        self.assertEqual(compat.compare_reports(current, self.baseline, "sample"), [])


if __name__ == "__main__":
    unittest.main()
