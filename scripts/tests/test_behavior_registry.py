"""Tests for the T004 editing-behavior registry validation in check_docs.py."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import check_docs  # noqa: E402

ENTRY = {
    "id": "EDIT-X",
    "operation": "组合确认提交",
    "expectation": "一次撤销回退整次提交",
    "source": "hypothesis",
    "test": "some_test_name",
    "word_observation": None,
}

OBSERVATION = {
    "version": "Word 2024 永久版",
    "sample_id": "DOCX-MIN-001",
    "date": "2026-10-17",
    "screenshot_sha256": "ab" * 32,
}


def registry(entries):
    return {"schema": 1, "registry": entries}


class BehaviorRegistryTest(unittest.TestCase):
    def run_registry(self, data, names):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "registry.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            return check_docs.validate_behavior_registry(path, names)

    def test_valid_hypothesis_passes(self):
        problems, count = self.run_registry(registry([ENTRY]), {"some_test_name"})
        self.assertEqual(problems, [])
        self.assertEqual(count, 1)

    def test_missing_file_fails(self):
        problems, _ = check_docs.validate_behavior_registry(
            Path("/nonexistent/registry.json"), set()
        )
        self.assertTrue(problems)

    def test_referenced_test_name_must_exist(self):
        problems, _ = self.run_registry(registry([ENTRY]), set())
        self.assertTrue(any("not found" in problem for problem in problems))

    def test_hypothesis_must_not_claim_a_word_observation(self):
        entry = dict(ENTRY, word_observation=OBSERVATION)
        problems, _ = self.run_registry(registry([entry]), {"some_test_name"})
        self.assertTrue(any("must not claim" in problem for problem in problems))

    def test_word_rule_needs_a_valid_observation(self):
        entry = dict(ENTRY, source="word")
        problems, _ = self.run_registry(registry([entry]), {"some_test_name"})
        self.assertTrue(any("word_observation" in problem for problem in problems))
        bad_date = dict(OBSERVATION, date="2026-13-40")
        entry = dict(ENTRY, source="word", word_observation=bad_date)
        problems, _ = self.run_registry(registry([entry]), {"some_test_name"})
        self.assertTrue(any("YYYY-MM-DD" in problem for problem in problems))
        bad_hash = dict(OBSERVATION, screenshot_sha256="xyz")
        entry = dict(ENTRY, source="word", word_observation=bad_hash)
        problems, _ = self.run_registry(registry([entry]), {"some_test_name"})
        self.assertTrue(any("SHA-256" in problem for problem in problems))

    def test_word_rule_with_full_observation_passes(self):
        entry = dict(ENTRY, source="word", word_observation=OBSERVATION)
        problems, count = self.run_registry(registry([entry]), {"some_test_name"})
        self.assertEqual(problems, [])
        self.assertEqual(count, 1)

    def test_duplicate_id_fails(self):
        problems, _ = self.run_registry(registry([ENTRY, dict(ENTRY)]), {"some_test_name"})
        self.assertTrue(any("duplicate" in problem for problem in problems))

    def test_rust_function_names_finds_plain_and_generic_tests(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "src"
            src.mkdir()
            (src / "a.rs").write_text(
                "fn alpha_test() {}\nfn beta_test<T>() {}\n", encoding="utf-8"
            )
            names = check_docs.rust_function_names([src])
        self.assertIn("alpha_test", names)
        self.assertIn("beta_test", names)

    def test_repo_registry_is_valid_and_every_test_exists(self):
        problems, count = check_docs.validate_behavior_registry(
            check_docs.DOCS / "behavior_registry.json",
            check_docs.rust_function_names(
                [check_docs.ROOT / "crates", check_docs.ROOT / "apps"]
            ),
        )
        self.assertEqual(problems, [])
        self.assertGreater(count, 0)


if __name__ == "__main__":
    unittest.main()
