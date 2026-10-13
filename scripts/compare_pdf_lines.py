#!/usr/bin/env python3
"""Compare line and page breaks of two PDFs (T004 geometric comparison).

Both files go through the same extractor, poppler `pdftotext -bbox-layout`,
so a reference exported by a fixed Word build and a JoyWord PDF are measured
with identical conventions. Reported per QUALITY §4:

- page count and lines per page (page breaks),
- the text of every line with whitespace removed (line breaks),
- the largest start-x and line-bottom deviation of matching lines, which must
  stay within the tolerance (default 0.5 pt, the Word-reference threshold).

`--dump file.pdf` prints the extracted pages and lines as JSON, the format in
which reference geometry is attached to registry evidence. Coordinates are
line and word boxes from the reader, not individual glyph origins.
"""
import argparse
import json
import re
import shutil
import subprocess
import sys
from html.parser import HTMLParser


class BBoxPages(HTMLParser):
    def __init__(self):
        super().__init__()
        self.pages = []
        self.word = None

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "page":
            self.pages.append({"width": float(attrs["width"]), "height": float(attrs["height"]), "lines": []})
        elif tag == "line" and self.pages:
            self.pages[-1]["lines"].append({
                "x_min": float(attrs["xmin"]),
                "x_max": float(attrs["xmax"]),
                "y_max": float(attrs["ymax"]),
                "words": [],
            })
        elif tag == "word":
            self.word = []

    def handle_endtag(self, tag):
        if tag == "word" and self.word is not None:
            self.pages[-1]["lines"][-1]["words"].append("".join(self.word))
            self.word = None

    def handle_data(self, data):
        if self.word is not None:
            self.word.append(data)


def extract(pdf):
    if not shutil.which("pdftotext"):
        sys.exit("ERROR: pdftotext (poppler-utils) is required")
    html = subprocess.run(
        ["pdftotext", "-bbox-layout", pdf, "-"], capture_output=True, text=True, check=True
    ).stdout
    parser = BBoxPages()
    parser.feed(html)
    for page in parser.pages:
        for line in page["lines"]:
            line["text"] = " ".join(line.pop("words"))
    return parser.pages


def compact(text):
    return re.sub(r"\s+", "", text)


def compare(reference, candidate, tolerance):
    """Returns (problems, worst_x, worst_y); problems is a list of strings."""
    problems = []
    worst_x = worst_y = 0.0
    if len(reference) != len(candidate):
        problems.append(f"page count {len(candidate)} != reference {len(reference)}")
    for page_number, (ref_page, got_page) in enumerate(zip(reference, candidate), start=1):
        ref_lines, got_lines = ref_page["lines"], got_page["lines"]
        if len(ref_lines) != len(got_lines):
            problems.append(f"page {page_number}: {len(got_lines)} lines != reference {len(ref_lines)}")
        for line_number, (ref, got) in enumerate(zip(ref_lines, got_lines), start=1):
            if compact(ref["text"]) != compact(got["text"]):
                problems.append(
                    f"page {page_number} line {line_number}: line break differs: "
                    f"{got['text']!r} != reference {ref['text']!r}"
                )
                break  # later lines on this page are shifted; report the first
            dx = abs(ref["x_min"] - got["x_min"])
            dy = abs(ref["y_max"] - got["y_max"])
            worst_x, worst_y = max(worst_x, dx), max(worst_y, dy)
            if dx > tolerance or dy > tolerance:
                problems.append(
                    f"page {page_number} line {line_number}: offset x {dx:.3f} pt, y {dy:.3f} pt"
                )
    return problems, worst_x, worst_y


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("pdfs", nargs="*")
    parser.add_argument("--tolerance", type=float, default=0.5)
    parser.add_argument("--dump", metavar="PDF")
    args = parser.parse_args()
    if args.dump:
        json.dump(extract(args.dump), sys.stdout, ensure_ascii=False, indent=1)
        print()
        return 0
    if len(args.pdfs) != 2:
        parser.error("give a reference PDF and a candidate PDF, or --dump PDF")
    problems, worst_x, worst_y = compare(extract(args.pdfs[0]), extract(args.pdfs[1]), args.tolerance)
    print(f"worst start-x deviation {worst_x:.4f} pt; worst line-bottom deviation {worst_y:.4f} pt; "
          f"tolerance {args.tolerance} pt")
    for problem in problems:
        print(f"DIFF: {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
