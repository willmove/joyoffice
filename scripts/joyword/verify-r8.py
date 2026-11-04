#!/usr/bin/env python3
"""Exercise the integrated JoyWord CLI against a mixed-media DOCX."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from zipfile import ZIP_DEFLATED, ZipFile


ROOT = Path(__file__).resolve().parents[2]
WORD = {"w": "http://schemas.openxmlformats.org/wordprocessingml/2006/main"}
MARKER = "JoyOffice R8 verification 测试"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(binary, *args, expected=0):
    result = subprocess.run([binary, *args], capture_output=True, text=True,
                            encoding="utf-8", timeout=90, check=False)
    require(result.returncode == expected,
            f"{args[0]} exited {result.returncode}, expected {expected}: {result.stderr}")
    return result


def verify(binary, source, work, independent_pdf):
    original = hashlib.sha256(source.read_bytes()).hexdigest()
    before = json.loads(run(binary, "inspect", "--session", "--", source).stdout)
    require(before["pages"] and "JoyWord" in before["text"], "Sample has no rendered text")
    require(run(binary, "text", "--session", source).stdout.strip() == before["text"].strip(),
            "Plain text and inspect disagree")

    output = work / "R8 文档.docx"
    result = json.loads(run(binary, "run", source, "--cmd", "caret.docEnd", "--cmd",
                            "text.insert=" + json.dumps({"text": MARKER}), "--save", output).stdout)
    require(MARKER in result["text"], "Edit did not reach the session")
    reopened = json.loads(run(binary, "inspect", "--session", output).stdout)
    require(MARKER in reopened["text"] and reopened["pages"], "Edit did not survive reopening")
    with ZipFile(output) as package:
        tree = ET.fromstring(package.read("word/document.xml"))
        text = "".join(node.text or "" for node in tree.findall(".//w:t", WORD))
        require(MARKER in text, "Edit is missing from the saved OOXML")
        require(tree.find(".//w:tbl", WORD) is not None, "Table was lost")
        require(any(name.startswith("word/media/") for name in package.namelist()), "Media was lost")

    saved = output.read_bytes()
    refusal = run(binary, "run", source, "--save", output, expected=1)
    require(refusal.stderr and output.read_bytes() == saved, "Existing output was changed")
    invalid = work / "invalid.docx"
    invalid.write_bytes(b"PK\x03\x04 truncated package")
    require(run(binary, "inspect", "--session", invalid, expected=1).stderr, "Invalid package was not diagnosed")
    require(hashlib.sha256(source.read_bytes()).hexdigest() == original, "Input DOCX was changed")

    png, pdf = work / "page.png", work / "document.pdf"
    run(binary, "render", output, png)
    run(binary, "convert", output, pdf)
    require(png.read_bytes().startswith(b"\x89PNG\r\n\x1a\n"), "Rendered page is not PNG")
    require(pdf.read_bytes().startswith(b"%PDF-"), "Export is not PDF")
    if independent_pdf:
        subprocess.run(["qpdf", "--check", pdf], check=True, timeout=30)
        extracted = subprocess.check_output(["pdftotext", pdf, "-"], encoding="utf-8", timeout=30)
        require("JoyOffice R8 verification" in extracted, "PDF text did not survive export")
    print(json.dumps({"checks": ["inspect", "text", "unicode edit", "reopen", "OOXML",
                                "no-clobber", "invalid DOCX", "render", "PDF"],
                      "input_sha256": original, "pages": len(reopened["pages"]),
                      "independent_pdf": independent_pdf}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=lambda path: Path(path).resolve())
    parser.add_argument("--sample", type=Path, default=ROOT / "assets/samples/mixed-media.docx")
    parser.add_argument("--out", type=Path, help="New directory for verification outputs")
    parser.add_argument("--independent-pdf", action="store_true", help="Require qpdf and pdftotext")
    parser.add_argument("--mutations", type=int, choices=range(1, 257), metavar="1..256",
                        help="Run a deterministic malformed DOCX campaign after the round trip")
    args = parser.parse_args()
    def verify_all(work):
        verify(args.binary, args.sample, work, args.independent_pdf)
        if args.mutations:
            verify_mutations(args.binary, args.sample, work, args.mutations)

    if args.out:
        args.out.mkdir(parents=True, exist_ok=False)
        verify_all(args.out)
    else:
        with tempfile.TemporaryDirectory(prefix="joyword-r8-") as work:
            verify_all(Path(work))


def verify_mutations(binary, source, work, count):
    rng = random.Random(20261104)
    package = source.read_bytes()
    with ZipFile(source) as archive:
        parts = {name: archive.read(name) for name in archive.namelist()}
    xml_parts = sorted(name for name in parts if name.endswith((".xml", ".rels")))
    accepted = 0
    mutant = work / "mutant.docx"
    for case in range(count):
        if case % 4 == 0:
            mutant.write_bytes(package[:rng.randrange(len(package))])
        else:
            name = rng.choice(xml_parts)
            data = parts[name]
            at = rng.randrange(len(data))
            replacement = data[:at] + rng.choice((b"<", b">", b"\xff", b"")) + data[at + 1:]
            with ZipFile(mutant, "w", ZIP_DEFLATED) as output:
                for part, value in parts.items():
                    output.writestr(part, replacement if part == name else value)
        result = subprocess.run([binary, "inspect", "--session", mutant], capture_output=True, text=True,
                                encoding="utf-8", timeout=10, check=False)
        require("failed internally" not in result.stderr and "panicked at" not in result.stderr,
                f"Mutation {case} triggered a panic: {result.stderr}")
        if result.returncode == 0:
            report = json.loads(result.stdout)
            require(report["pages"] and isinstance(report["text"], str), f"Invalid report at mutation {case}")
            accepted += 1
        else:
            require(result.returncode == 1 and result.stderr, f"Unclean rejection at mutation {case}")
    print(json.dumps({"mutations": count, "accepted": accepted, "rejected": count - accepted}))


if __name__ == "__main__":
    main()
