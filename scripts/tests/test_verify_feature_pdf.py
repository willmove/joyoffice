"""Feature-scene read-back rejects missing text, leaked revisions and page loss."""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import verify_feature_pdf as verify


class FeaturePdfTests(unittest.TestCase):
    def test_compact_text_ignores_reader_spaces_and_a_trailing_form_feed(self):
        scenario = {
            "pdf": "sample.pdf",
            "pages": 2,
            "must_contain": ["甲栏", "乙栏"],
            "must_absent": ["删去"],
        }
        extracted = "甲 栏\n乙栏\f\f"
        self.assertEqual(verify.compare(scenario, extracted), [])
        self.assertEqual(verify.page_texts(extracted), ["甲 栏\n乙栏", ""])

    def test_missing_phrase_and_page_loss_are_reported(self):
        scenario = {"pdf": "sample.pdf", "pages": 2, "must_contain": ["偶数"], "must_absent": []}
        errors = verify.compare(scenario, "同页\f")
        self.assertTrue(any("missing" in error for error in errors))
        self.assertTrue(any("pages" in error for error in errors))

    def test_revision_text_that_should_be_hidden_is_reported(self):
        scenario = {
            "pdf": "revisions-final.pdf",
            "pages": 1,
            "must_contain": ["新增"],
            "must_absent": ["删去"],
        }
        errors = verify.compare(scenario, "留下新增删去\f")
        self.assertEqual(errors, ["revisions-final.pdf: unexpected '删去'"])

    def test_a_blank_middle_page_still_counts(self):
        scenario = {"pdf": "sections.pdf", "pages": 3, "must_contain": ["偶数"], "must_absent": []}
        self.assertEqual(verify.compare(scenario, "同页续写\f\f偶数\f"), [])


if __name__ == "__main__":
    unittest.main()
