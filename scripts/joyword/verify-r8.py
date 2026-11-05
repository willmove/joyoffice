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
from zipfile import ZIP_DEFLATED, ZIP_STORED, ZipFile


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


def verify_legacy_commands(binary, work, independent_pdf):
    fixtures = ROOT / "tests/fixtures"
    template = fixtures / "mail-merge/template.docx"
    csv = fixtures / "mail-merge/recipients.csv"
    inputs = {path: path.read_bytes() for path in (template, csv)}
    merge = ("merge", "--template", template, "--data", csv)
    preview = json.loads(run(binary, *merge, "--preview-row", "3").stdout)
    require(preview["schema_version"] == 1 and preview["writes"] == 0
            and preview["values"]["单位"] == "南山\n研究组", "Invalid merge preview")
    run(binary, *merge, "--preview-row", "4", expected=2)
    run(binary, *merge, "--map", "姓名=Missing", "--output-parent", work, expected=2)
    batch = json.loads(run(binary, *merge, "--output-parent", work).stdout)
    require(batch["records"] == 3, "Merge did not export three records")
    for index, (name, output) in enumerate(zip(("张三", "李四", "王五"), batch["files"], strict=True), 1):
        text = json.loads(run(binary, "inspect", output).stdout)["text"]
        require(Path(output).name == f"letter-{index:04d}.docx" and name in text and "{{" not in text,
                "Merged DOCX did not reopen with the expected recipient")
    require(all(path.read_bytes() == data for path, data in inputs.items()), "Merge changed an input")

    source = work / "源格式.txt"
    source.write_text("Portable text & literal <example>", encoding="utf-8")
    destination = work / "exchange.html"
    exchange = ("exchange", source, "--from", "txt", "--to", "html")
    plan = json.loads(run(binary, *exchange, "--plan").stdout)
    require(plan["schema_version"] == 1 and plan["writes"] == 0 and plan["losses"],
            "Exchange plan did not disclose losses")
    run(binary, *exchange, "--output", destination, expected=2)
    require(not destination.exists(), "Exchange wrote without acceptance")
    run(binary, *exchange, "--output", destination, "--accept-losses")
    html = destination.read_bytes()
    require(b"&amp; literal &lt;example&gt;" in html, "Exchange did not escape literal input")
    run(binary, *exchange, "--output", destination, "--accept-losses", expected=2)
    require(destination.read_bytes() == html, "Exchange overwrote an existing file")
    hidden = work / "hidden.html"
    hidden.write_text('<span title=">" hidden>Private text</span>', encoding="utf-8")
    run(binary, "exchange", hidden, "--from", "html", "--to", "txt", "--plan", expected=2)

    docx = work / "portable.docx"
    run(binary, "exchange", source, "--from", "txt", "--to", "docx",
        "--output", docx, "--accept-losses")
    font = ROOT / "assets/fonts/SourceSans3-Regular.ttf"
    face = f"{font}:0"
    directory = work / "explicit-font"
    render = ("render", docx, "--font", face, "--output", directory)
    raster = json.loads(run(binary, *render).stdout)
    image = directory / "page-1.png"
    png = image.read_bytes()
    require(raster["dpi"] == 144 and raster["font_face"] == 0
            and raster["font_sha256"] == hashlib.sha256(font.read_bytes()).hexdigest()
            and png.startswith(b"\x89PNG\r\n\x1a\n"), "Invalid explicit-font render")
    run(binary, *render, expected=2)
    require(image.read_bytes() == png, "Render overwrote an existing page")
    run(binary, "render", template, "--font", face, "--output", work / "missing-glyph", expected=2)
    require(not (work / "missing-glyph").exists(), "Missing glyph produced an image")

    archived = work / "archive.pdf"
    duplicate = work / "archive-copy.pdf"
    convert = ("convert", docx, "--to", "pdfa-2b", "--font", face, "--archive-year", "2026")
    pdf = json.loads(run(binary, *convert, "--output", archived).stdout)
    run(binary, *convert, "--output", duplicate)
    archive = archived.read_bytes()
    require(pdf["target"] == "pdfa-2b" and archive.startswith(b"%PDF-1.7")
            and archive == duplicate.read_bytes(), "Archive changed between identical conversions")
    run(binary, *convert, "--output", archived, expected=2)
    require(archived.read_bytes() == archive, "Convert overwrote an existing PDF")
    if independent_pdf:
        subprocess.run(["qpdf", "--check", archived], check=True, timeout=30)
        extracted = subprocess.check_output(["pdftotext", archived, "-"], encoding="utf-8", timeout=30)
        require("Portable text & literal <example>" in extracted, "Archive lost text")
    print(json.dumps({"legacy_checks": ["merge preview and batch", "exchange loss gate",
          "explicit face and missing glyph", "deterministic PDF/A-2b", "no-clobber"]}))


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
        verify_legacy_commands(args.binary, work, args.independent_pdf)
        verify_epub(args.binary, work)
        verify_page_setup(args.binary, args.sample, work)
        if args.mutations:
            verify_mutations(args.binary, args.sample, work, args.mutations)

    if args.out:
        args.out.mkdir(parents=True, exist_ok=False)
        verify_all(args.out)
    else:
        with tempfile.TemporaryDirectory(prefix="joyword-r8-") as work:
            verify_all(Path(work))


def verify_epub(binary, work):
    fixtures = ROOT / "tests/fixtures"
    xhtml = "{http://www.w3.org/1999/xhtml}"
    options = ("--title", "中文 & EPUB", "--language", "zh-CN", "--modified", "2026-11-05T10:30:00Z")
    for source in (fixtures / "epub/book.docx", fixtures / "mail-merge/template.docx"):
        original = source.read_bytes()
        command = ("epub", source, *options)
        output = work / f"{source.stem}.epub"
        plan = json.loads(run(binary, *command, "--plan").stdout)
        require(plan["schema_version"] == 1 and plan["writes"] == 0 and plan["losses"],
                "EPUB plan did not disclose losses")
        run(binary, *command, "--output", output, expected=2)
        require(not output.exists(), "EPUB wrote without acceptance")
        result = json.loads(run(binary, *command, "--output", output, "--accept-losses").stdout)
        original_output = output.read_bytes()
        require(result["writes"] == 1 and result["bytes"] == len(original_output),
                "EPUB report disagrees with file")
        duplicate = work / f"{source.stem}-copy.epub"
        run(binary, *command, "--output", duplicate, "--accept-losses")
        require(duplicate.read_bytes() == original_output, "EPUB changed with identical input")
        run(binary, *command, "--output", output, "--accept-losses", expected=2)
        require(output.read_bytes() == original_output and source.read_bytes() == original,
                "EPUB overwrote an existing file")
        with ZipFile(output) as epub, ZipFile(source) as docx:
            mime = epub.infolist()[0]
            require(mime.filename == "mimetype" and mime.compress_type == ZIP_STORED
                    and not mime.extra and epub.read(mime) == b"application/epub+zip"
                    and epub.testzip() is None, "Invalid EPUB container")
            content = ET.fromstring(epub.read("EPUB/content.xhtml"))
            nav = ET.fromstring(epub.read("EPUB/nav.xhtml"))
            ncx = ET.fromstring(epub.read("EPUB/toc.ncx"))
            targets = [a.attrib["href"] for a in nav.iter(xhtml + "a")]
            ids = {element.get("id") for element in content.iter()}
            require(targets == [e.attrib["src"] for e in ncx.iter() if e.tag.endswith("}content")]
                    and all(t == "content.xhtml" or t.partition("#")[2] in ids for t in targets),
                    "EPUB navigation targets disagree")
            original_xml = ET.fromstring(docx.read("word/document.xml"))
            paragraphs = content.findall(f".//{xhtml}body//*")
            tags = {xhtml + "p", *(xhtml + f"h{i}" for i in range(1, 7))}
            original_text = [
                "".join(e.text or "" for e in p.findall(".//w:t", WORD))
                for p in original_xml.findall(".//w:p", WORD)
            ]
            require(["".join(e.itertext()) for e in paragraphs if e.tag in tags] == original_text,
                    "EPUB paragraph text or order changed")
            require(sorted(epub.read(n) for n in epub.namelist() if n.startswith("EPUB/images/")) ==
                    sorted(docx.read(n) for n in docx.namelist() if n.startswith("word/media/")),
                    "EPUB image bytes changed")
    reviewed = fixtures / "feature/DOCX-FEAT-T300-REVIEW.docx"
    run(binary, "epub", reviewed, "--plan", *options, expected=2)
    run(binary, "epub", fixtures / "epub/book.docx", "--plan", "--modified",
        "2026-02-29T10:30:00Z", expected=2)
    unknown = work / "epub-unknown.docx"
    with ZipFile(fixtures / "epub/book.docx") as source, ZipFile(unknown, "w", ZIP_DEFLATED) as output:
        for name in source.namelist():
            data = source.read(name)
            if name == "word/document.xml":
                require(b"<w:body>" in data, "EPUB fixture has no body")
                data = data.replace(b"<w:body>", b"<w:body><w:unknownBody/>", 1)
            output.writestr(name, data)
    refusal = run(binary, "epub", unknown, "--plan", expected=2)
    require("restricted or unsupported DOCX" in refusal.stderr, "Unknown body was not diagnosed")
    print(json.dumps({"epub_checks": ["review and no-clobber", "determinism", "text and images",
                                     "navigation and ZIP", "revisions, unknown body and invalid metadata"]}))


def verify_page_setup(binary, source, work):
    document = work / "page-setup.docx"
    geometry = {"width": 720, "height": 540, "landscape": True, "left": 54, "columns": 2}
    layout = "layout.pageSetup=" + json.dumps({"geometry": geometry})
    result = json.loads(run(binary, "run", source, "--cmd", layout, "--save", document).stdout)
    zoom = json.loads(run(binary, "run", document, "--cmd", 'view.zoom={"value":175}').stdout)
    require(result["pages"] == zoom["pages"] and result["pages"][0] == {"width": 720, "height": 540},
            "Page size changed after reopening or zooming")
    with ZipFile(document) as archive:
        xml = ET.fromstring(archive.read("word/document.xml"))
        section = xml.find(".//w:body/w:sectPr", WORD)
        attr = f'{{{WORD["w"]}}}'
        require(section.find("w:cols", WORD).get(attr + "num") == "2"
                and section.find("w:pgMar", WORD).get(attr + "left") == "1080",
                "Page setup did not survive in OOXML")
    run(binary, "convert", document, work / "page-setup.pdf")
    print(json.dumps({"page_setup": result["pages"], "zoom_percent": 175, "columns": 2}))


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
