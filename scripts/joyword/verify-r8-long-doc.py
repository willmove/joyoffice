"""Measure JoyWord's 1,000-page DOCX CLI open, edit, save, reopen, and render on macOS."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

ROOT = Path(__file__).resolve().parents[2]
NAMESPACE = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
PAGE_COUNT = 1000
KEYS = "adghk"
TAIL = "THOUSAND-PAGE-TAIL."


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def make_fixture(path):
    body = []
    for page in range(1, PAGE_COUNT + 1):
        page_break = "<w:pPr><w:pageBreakBefore/></w:pPr>" if page > 1 else ""
        label = f"JoyWord performance sample {page:04d}."
        if page == PAGE_COUNT:
            label += " This English paragraph wraps naturally to multiple lines. " * 4 + TAIL
        body.append(f"<w:p>{page_break}<w:r><w:t>{label}</w:t></w:r></w:p>")
    xml = (
        f'<w:document xmlns:w="{NAMESPACE}"><w:body>{"".join(body)}'
        '<w:sectPr><w:pgSz w:w="12240" w:h="15840"/>'
        '<w:pgMar w:top="1440" w:bottom="1440" w:left="1440" w:right="1440"/>'
        '</w:sectPr></w:body></w:document>'
    )
    template = ROOT / "tests/fixtures/docx/DOCX-MIN-001.docx"
    with ZipFile(template) as original, ZipFile(path, "x") as fixture:
        for name in original.namelist():
            payload = xml.encode() if name == "word/document.xml" else original.read(name)
            entry = ZipInfo(name, (2026, 11, 5, 0, 0, 0))
            fixture.writestr(entry, payload, compress_type=ZIP_DEFLATED)


def timed(cli, *args):
    start = time.perf_counter()
    proc = subprocess.run(
        ["/usr/bin/time", "-l", cli, *args],
        capture_output=True, encoding="utf-8", timeout=120,
    )
    wall_ms = (time.perf_counter() - start) * 1000
    if proc.returncode:
        raise RuntimeError(f"{args[0]} failed ({proc.returncode}): {proc.stderr}")
    rss = re.search(r"^\s*(\d+)\s+maximum resident set size", proc.stderr, re.M)
    if rss is None:
        raise RuntimeError("macOS time did not report maximum resident set size")
    return json.loads(proc.stdout), {"wallMs": round(wall_ms, 3), "maxRssBytes": int(rss[1])}


def verify(cli, work, attempts):
    source = work / "千页 R8.docx"
    make_fixture(source)
    source_hash = sha256(source)
    samples = []
    for attempt in range(attempts):
        doc, png = work / f"edit-{attempt}.docx", work / f"page-{attempt}.png"
        opened, initial = timed(cli, "inspect", "--session", source)
        command = ["run", source, "--cmd", "caret.docEnd"]
        for character in KEYS:
            command.extend(["--cmd", "text.insert=" + json.dumps({"text": character})])
        edited, edit_save = timed(cli, *command, "--save", doc)
        reopened, reopen = timed(cli, "inspect", "--session", doc)
        rendered, raster = timed(cli, "render", doc, png, "--page", str(PAGE_COUNT), "--scale", "1")
        if any(len(report["pages"]) != PAGE_COUNT for report in (opened, edited, reopened)):
            raise RuntimeError("DOCX page count changed")
        if not edited["text"].endswith(TAIL + KEYS) or reopened["text"] != edited["text"]:
            raise RuntimeError("Last paragraph did not survive reopening")
        with ZipFile(doc) as package:
            main = ET.fromstring(package.read("word/document.xml"))
        text = "".join(run.text or "" for run in main.iter(f"{{{NAMESPACE}}}t"))
        if not text.endswith(TAIL + KEYS) or rendered["pages"] != PAGE_COUNT:
            raise RuntimeError("OOXML text or render report did not match")
        if not png.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"):
            raise RuntimeError("Last page was not written as a PNG")
        samples.append(dict(open=initial, editSave=edit_save, reopen=reopen, renderLastPage=raster,
                            openLayoutMs=opened["layout_ms"], finalEditLayoutMs=edited["layout_ms"]))
    if sha256(source) != source_hash:
        raise RuntimeError("Source DOCX changed")
    summary = {}
    for phase in ("open", "editSave", "reopen", "renderLastPage"):
        summary[phase] = dict(medianWallMs=statistics.median(s[phase]["wallMs"] for s in samples),
                              peakRssBytes=max(s[phase]["maxRssBytes"] for s in samples))
    summary["medianOpenLayoutMs"] = statistics.median(s["openLayoutMs"] for s in samples)
    summary["medianLastEditLayoutMs"] = statistics.median(s["finalEditLayoutMs"] for s in samples)
    return dict(fixtureSha256=source_hash, binarySha256=sha256(cli), pages=PAGE_COUNT,
                runs=samples, summary=summary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--runs", default=5, type=int)
    args = parser.parse_args()
    if sys.platform != "darwin" or not 1 <= args.runs <= 20:
        parser.error("requires macOS and 1 to 20 runs")
    with tempfile.TemporaryDirectory(prefix="joyword-long-doc-") as work:
        report = verify(args.cli.resolve(strict=True), Path(work), args.runs)
    with args.evidence.open("x", encoding="utf-8") as evidence:
        json.dump(report, evidence, indent=2)
        evidence.write("\n")


if __name__ == "__main__":
    main()
