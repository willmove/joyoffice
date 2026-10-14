"""Ensure readback verification rejects shifted pages, breaks and geometry."""
import copy
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
import verify_document_pdf as verify


class DocumentPdfTests(unittest.TestCase):
    def setUp(self):
        self.expected = {"pages": [
            {"width": 100.0, "height": 200.0, "lines": [
                {"text": "中文 AV", "x_pt": 10.0, "width_pt": 30.0, "baseline_pt": 20.0},
                {"text": "第二行", "x_pt": 10.0, "width_pt": 36.0, "baseline_pt": 38.0}]},
            {"width": 100.0, "height": 200.0, "lines": [
                {"text": "下一页", "x_pt": 10.0, "width_pt": 36.0, "baseline_pt": 20.0}]}]}
        self.actual = copy.deepcopy(self.expected["pages"])
        for page in self.actual:
            for line in page["lines"]:
                line.update(x_min=line["x_pt"], x_max=line["x_pt"] + line["width_pt"],
                            y_max=line["baseline_pt"] + 2.0)

    def test_matching_pages_pass_and_reader_spaces_are_normalized(self):
        self.actual[0]["lines"][0]["text"] = "中文AV"
        self.assertEqual(verify.compare(self.expected, self.actual), [])

    def test_page_loss_and_page_size_changes_are_detected(self):
        self.assertTrue(verify.compare(self.expected, self.actual[:1]))
        self.actual[1]["height"] += 1
        self.assertTrue(any("height" in e for e in verify.compare(self.expected, self.actual)))

    def test_shifted_second_page_is_detected(self):
        self.actual[1]["lines"][0]["y_max"] += 18
        self.assertTrue(any("baseline" in e for e in verify.compare(self.expected, self.actual)))

    def test_text_line_break_and_horizontal_errors_are_detected(self):
        for field, value, message in (("text", "changed", "text or break"),
                                       ("x_min", 11.0, "start x"),
                                       ("x_max", 41.0, "end x")):
            with self.subTest(field=field):
                actual = copy.deepcopy(self.actual)
                actual[0]["lines"][0][field] = value
                self.assertTrue(any(message in e for e in verify.compare(self.expected, actual)))
        self.actual[0]["lines"].pop()
        self.assertTrue(any("line count" in e for e in verify.compare(self.expected, self.actual)))


if __name__ == "__main__":
    unittest.main()
