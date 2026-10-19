#!/usr/bin/env python3
"""Read feature-scene PDFs back with poppler.

Input is the directory written by `cargo run -p joyword-cli --example
feature_scenes`. Each scenario names a PDF, the page count, phrases that
must survive, and phrases that must not. Extraction uses `pdftotext -enc
UTF-8`. Whitespace is ignored because readers rebuild gaps from glyph
positions.

A passing run shows that the PDF carries the scene text. It is not a visual
check and it does not compare the file with Word or WPS.
"""

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path


def compact(text):
    return re.sub(r"\s+", "", text)


def page_texts(extracted):
    """Split poppler pages. A trailing form feed is not an extra page."""
    parts = extracted.split("\f")
    if parts and parts[-1] == "":
        parts.pop()
    return parts


def compare(scenario, extracted):
    errors = []
    name = scenario.get("pdf", "pdf")
    text = compact(extracted)
    pages = page_texts(extracted)
    wanted = scenario.get("pages")
    if isinstance(wanted, int) and len(pages) != wanted:
        errors.append(f"{name}: {len(pages)} pages, expected {wanted}")
    for phrase in scenario.get("must_contain", []):
        if compact(phrase) not in text:
            errors.append(f"{name}: missing {phrase!r}")
    for phrase in scenario.get("must_absent", []):
        if compact(str(phrase)) and compact(phrase) in text:
            errors.append(f"{name}: unexpected {phrase!r}")
    return errors


def main(argv):
    if len(argv) != 2:
        return "usage: verify_feature_pdf.py <out-dir>"
    out = Path(argv[1])
    expected_path = out / "expected.json"
    try:
        expected = json.loads(expected_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return f"ERROR: cannot read {expected_path}: {error}"
    scenarios = expected.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        return "ERROR: expected.json needs a non-empty scenarios list"
    if shutil.which("pdftotext") is None:
        return "ERROR: pdftotext is not installed"
    errors = []
    for scenario in scenarios:
        pdf = out / str(scenario.get("pdf", ""))
        result = subprocess.run(
            ["pdftotext", "-enc", "UTF-8", str(pdf), "-"],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or f"exit {result.returncode}"
            errors.append(f"{pdf.name}: pdftotext failed: {detail}")
            continue
        errors.extend(compare(scenario, result.stdout))
    if errors:
        return "\n".join(errors)
    print(f"reader: poppler pdftotext -enc UTF-8; {len(scenarios)} scenarios")
    return None


if __name__ == "__main__":
    failure = main(sys.argv)
    if failure:
        print(failure, file=sys.stderr)
        sys.exit(1)
