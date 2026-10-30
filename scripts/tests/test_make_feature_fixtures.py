"""The feature-fixture generator is deterministic and matches the manifest."""

import hashlib
import importlib.util
import io
import pathlib
import unittest
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "make_feature_fixtures", ROOT / "scripts" / "make_feature_fixtures.py"
)
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)


class FeatureFixtureGeneratorTests(unittest.TestCase):
    def test_two_generations_match_and_use_fixed_zip_metadata(self):
        first = generator.documents()
        second = generator.documents()
        self.assertEqual(list(first), list(generator.CASES))
        self.assertEqual(first, second)
        for case_id, blob in first.items():
            members = [name for name, _data in generator.case_parts(case_id)]
            with zipfile.ZipFile(io.BytesIO(blob)) as archive:
                infos = archive.infolist()
                self.assertEqual([info.filename for info in infos], members)
                for info in infos:
                    self.assertEqual(info.date_time, (1980, 1, 1, 0, 0, 0))
                    self.assertEqual(info.compress_type, zipfile.ZIP_STORED)
                    self.assertFalse(info.flag_bits & 0x1)
                    self.assertFalse(info.flag_bits & 0x8)
            self.assertTrue(blob.startswith(b"PK"))

    def test_committed_bytes_and_manifest_hashes_match(self):
        import json

        fresh = generator.documents()
        directory = ROOT / "tests" / "fixtures" / "feature"
        problems = generator.check_committed(directory)
        self.assertEqual(problems, [])
        manifest = json.loads((ROOT / "tests" / "fixtures" / "manifest.json").read_text(encoding="utf-8"))
        registered = {case["id"]: case for case in manifest["cases"]}
        for case_id, blob in fresh.items():
            case = registered[case_id]
            digest = hashlib.sha256(blob).hexdigest()
            self.assertEqual(case["sha256"], digest)
            self.assertEqual(case["kind"], "format_case")
            self.assertEqual(case["path"], f"feature/{case_id}.docx")
            self.assertEqual(case["document"], f"feature/{case_id}.docx")
            self.assertEqual(case["origin"], generator.ORIGIN)
            self.assertEqual(case["license"], "MIT")
            self.assertEqual(case["status"], "planned")
            self.assertEqual(case["evidence"], [])
            self.assertEqual(case["reference_profile"]["status"], "unverified")
            self.assertTrue(case["requirements"])
            self.assertTrue(case["expected"])
            committed = (directory / f"{case_id}.docx").read_bytes()
            self.assertEqual(hashlib.sha256(committed).hexdigest(), digest)

    def test_png_helpers_are_tiny_distinct_and_stable(self):
        pictures = [
            generator.png_rgb(220, 40, 40),
            generator.png_rgb(40, 160, 60),
            generator.png_rgb(40, 80, 200),
            generator.png_rgb(230, 180, 20),
        ]
        self.assertEqual(len(set(pictures)), 4)
        for blob in pictures:
            self.assertTrue(blob.startswith(b"\x89PNG\r\n\x1a\n"))
            self.assertLess(len(blob), 128)
        self.assertEqual(generator.png_rgb(1, 2, 3), generator.png_rgb(1, 2, 3))


if __name__ == "__main__":
    unittest.main()
