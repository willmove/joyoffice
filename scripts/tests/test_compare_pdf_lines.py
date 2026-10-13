"""Logic tests for compare_pdf_lines.py; no PDF reader needed."""
import importlib.util
import pathlib
import unittest

SPEC = importlib.util.spec_from_file_location(
    "compare_pdf_lines", pathlib.Path(__file__).resolve().parents[1] / "compare_pdf_lines.py"
)
compare_pdf_lines = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(compare_pdf_lines)


def page(*lines):
    return {"width": 595.0, "height": 842.0, "lines": [
        {"text": text, "x_min": x, "x_max": x + 100, "y_max": y} for text, x, y in lines
    ]}


REFERENCE = [page(("甲方 乙方", 72.0, 90.0), ("第二行", 72.0, 108.0)), page(("第二页", 72.0, 90.0))]


class CompareTests(unittest.TestCase):
    def test_identical_layout_passes(self):
        problems, worst_x, worst_y = compare_pdf_lines.compare(REFERENCE, REFERENCE, 0.5)
        self.assertEqual(problems, [])
        self.assertEqual((worst_x, worst_y), (0.0, 0.0))

    def test_spaces_inferred_by_readers_do_not_count(self):
        candidate = [page(("甲方乙方", 72.0, 90.0), ("第二行", 72.0, 108.0)), REFERENCE[1]]
        self.assertEqual(compare_pdf_lines.compare(REFERENCE, candidate, 0.5)[0], [])

    def test_line_break_difference_is_reported_once_per_page(self):
        candidate = [page(("甲方", 72.0, 90.0), ("乙方", 72.0, 108.0), ("第二行", 72.0, 126.0)), REFERENCE[1]]
        problems, _, _ = compare_pdf_lines.compare(REFERENCE, candidate, 0.5)
        self.assertTrue(any("3 lines != reference 2" in p for p in problems))
        self.assertEqual(sum("line break differs" in p for p in problems), 1)

    def test_page_break_and_offsets_beyond_tolerance(self):
        moved = [page(("甲方 乙方", 72.6, 90.0), ("第二行", 72.0, 108.2))]
        problems, worst_x, worst_y = compare_pdf_lines.compare(REFERENCE, moved, 0.5)
        self.assertTrue(any("page count 1 != reference 2" in p for p in problems))
        self.assertTrue(any("offset x 0.600" in p for p in problems))
        self.assertAlmostEqual(worst_x, 0.6)
        self.assertAlmostEqual(worst_y, 0.2)


if __name__ == "__main__":
    unittest.main()
