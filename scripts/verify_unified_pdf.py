#!/usr/bin/env python3
"""Read a unified-scene PDF back with an independent reader (T003).

Input: the directory written by `cargo run -p joyword-pdf --example
unified_scene`, holding scene.pdf and expected.json. Backends:

- poppler `pdftotext -bbox-layout` (CI on Linux): extracted text per line and
  the line's start and end x in points are compared with the scene.
- macOS PDFKit through `swift` (local fallback): extracted text only.

Text is compared with whitespace removed, because readers infer spaces from
glyph gaps; the Unicode mapping of every glyph is what must survive. A
passing run proves the PDF carries the scene's text and positions; it is not
a visual or Word-compatibility check.
"""
import json
import re
import shutil
import subprocess
import sys
import tempfile
from html.parser import HTMLParser
from pathlib import Path

START_TOLERANCE_PT = 0.01  # QUALITY §4: unit conversion of one scene
END_TOLERANCE_PT = 0.05    # reader rebuilds line ends from PDF width tables

SWIFT = r"""
import Foundation
import PDFKit
let url = URL(fileURLWithPath: CommandLine.arguments[1])
guard let document = PDFDocument(url: url), let page = document.page(at: 0) else { exit(1) }
print(page.string ?? "")
"""


def compact(text):
    return re.sub(r"\s+", "", text)


class BBoxLines(HTMLParser):
    def __init__(self):
        super().__init__()
        self.lines = []
        self.word = None

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "line":
            self.lines.append({"x_min": float(attrs["xmin"]), "x_max": float(attrs["xmax"]), "words": []})
        elif tag == "word":
            self.word = []

    def handle_endtag(self, tag):
        if tag == "word" and self.word is not None:
            self.lines[-1]["words"].append("".join(self.word))
            self.word = None

    def handle_data(self, data):
        if self.word is not None:
            self.word.append(data)


def poppler_lines(pdf):
    html = subprocess.run(
        ["pdftotext", "-bbox-layout", str(pdf), "-"], capture_output=True, text=True, check=True
    ).stdout
    parser = BBoxLines()
    parser.feed(html)
    return [
        {"text": " ".join(line["words"]), "x_min": line["x_min"], "x_max": line["x_max"]}
        for line in parser.lines
    ]


def pdfkit_lines(pdf):
    with tempfile.TemporaryDirectory() as directory:
        script = Path(directory) / "extract.swift"
        script.write_text(SWIFT, encoding="utf-8")
        text = subprocess.run(
            ["swift", str(script), str(pdf)], capture_output=True, text=True, check=True
        ).stdout
    return [{"text": line} for line in text.splitlines() if line.strip()]


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: verify_unified_pdf.py <out-dir>")
    out = Path(sys.argv[1])
    expected = json.loads((out / "expected.json").read_text(encoding="utf-8"))
    pdf = out / "scene.pdf"
    if shutil.which("pdftotext"):
        backend, lines = "poppler pdftotext", poppler_lines(pdf)
    elif shutil.which("swift") and sys.platform == "darwin":
        backend, lines = "macOS PDFKit", pdfkit_lines(pdf)
    else:
        sys.exit("ERROR: no independent PDF reader (install poppler-utils)")

    errors = []
    wanted = expected["lines"]
    if len(lines) != len(wanted):
        errors.append(f"{len(lines)} lines extracted, {len(wanted)} expected")
    worst_start = worst_end = 0.0
    for number, (got, want) in enumerate(zip(lines, wanted), start=1):
        if compact(got["text"]) != compact(want["text"]):
            errors.append(f"line {number}: text {got['text']!r} != {want['text']!r}")
        if "x_min" in got:
            start = abs(got["x_min"] - want["x_pt"])
            end = abs(got["x_max"] - (want["x_pt"] + want["width_pt"]))
            worst_start, worst_end = max(worst_start, start), max(worst_end, end)
            if start > START_TOLERANCE_PT:
                errors.append(f"line {number}: starts at {got['x_min']} pt, scene {want['x_pt']} pt")
            if end > END_TOLERANCE_PT:
                errors.append(f"line {number}: ends {end:.4f} pt away from the scene")
    if expected["missing_glyphs"]:
        errors.append(f"{expected['missing_glyphs']} clusters had no glyph in the font")

    print(f"reader: {backend}; font sha256 {expected['font_sha256']} face {expected['face']}")
    print(f"lines: {len(lines)}; worst start deviation {worst_start:.4f} pt; worst end deviation {worst_end:.4f} pt")
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
