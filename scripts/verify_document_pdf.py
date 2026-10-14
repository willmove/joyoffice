#!/usr/bin/env python3
"""C05 readback of document_flow pages; never lays out or reshapes text.

Poppler checks all page boxes, line/page breaks, x positions, visible widths
and relative baselines (a single font throughout). --glyph-baselines also
requires pdfplumber and checks every extracted character's absolute baseline
against the scene, including the first line and subsequent pages.
"""
import argparse
import json
from pathlib import Path
import sys

from compare_pdf_lines import compact, extract


def compare(expected, actual):
    errors = []
    wanted = expected["pages"]
    if len(wanted) != len(actual):
        errors.append(f"page count {len(actual)} != {len(wanted)}")
    # The reader reports font boxes, not baseline origins. One fixed font
    # allows relative baseline checking, without pretending boxes are origins.
    baseline_offset = None
    for number, (want, got) in enumerate(zip(wanted, actual), 1):
        for dim in ("width", "height"):
            if abs(want[dim] - got[dim]) > 0.01:
                errors.append(f"page {number}: {dim}")
        if len(want["lines"]) != len(got["lines"]):
            errors.append(f"page {number}: line count")
        for index, (wl, gl) in enumerate(zip(want["lines"], got["lines"]), 1):
            prefix = f"page {number} line {index}"
            if compact(wl["text"]) != compact(gl["text"]):
                errors.append(f"{prefix}: text or break")
            if abs(wl["x_pt"] - gl["x_min"]) > 0.01:
                errors.append(f"{prefix}: start x")
            if abs(wl["x_pt"] + wl["width_pt"] - gl["x_max"]) > 0.05:
                errors.append(f"{prefix}: end x")
            offset = gl["y_max"] - wl["baseline_pt"]
            if baseline_offset is None:
                baseline_offset = offset
            if abs(offset - baseline_offset) > 0.01:
                errors.append(f"{prefix}: relative baseline")
    return errors


def glyph_baselines(pdf, expected):
    import pdfplumber  # optional independent absolute-origin check
    errors = []
    worst = 0.0
    count = 0
    with pdfplumber.open(pdf) as doc:
        if len(doc.pages) != len(expected["pages"]):
            errors.append("glyph reader page count")
        for number, (page, want) in enumerate(zip(doc.pages, expected["pages"]), 1):
            expected_ys = [line["baseline_pt"] for line in want["lines"]]
            seen = set()
            for char in page.chars:
                # PDF-space y increases up, scene-space y increases down.
                actual = float(page.height) - char["matrix"][5]
                index, delta = min(enumerate(abs(actual - y) for y in expected_ys), key=lambda p: p[1])
                worst = max(worst, delta)
                seen.add(index)
                count += 1
                if delta > 0.01:
                    errors.append(f"page {number}: character baseline deviation {delta:.6f} pt")
            if len(seen) != len(expected_ys):
                errors.append(f"page {number}: missing line baselines")
    print(f"pdfplumber: {count} characters; worst absolute baseline deviation {worst:.6f} pt")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--glyph-baselines", action="store_true")
    args = parser.parse_args()
    expected = json.loads((args.directory / "expected.json").read_text(encoding="utf-8"))
    pdf = args.directory / "scene.pdf"
    errors = compare(expected, extract(str(pdf)))
    if args.glyph_baselines:
        errors.extend(glyph_baselines(pdf, expected))
    print(f"poppler: {len(expected['pages'])} expected pages; {len(errors)} errors; font {expected['font_sha256']} face {expected['face']}")
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
