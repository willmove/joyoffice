"""Verify grouped and linked text boxes and deferred endnotes in the desktop editor."""

import argparse
import math
import subprocess
import tempfile
import json
from pathlib import Path
import re
import runpy
import secrets
import shutil
import sys
from xml.etree import ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import app, digest, port, wait, word_request
from verify_save_changes import word_keys

HELPERS = runpy.run_path(str(Path(__file__).with_name("check-section-reflow.py")))
DOCX = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))
BASE = "http://schemas.microsoft.com/office/word/2010/"
WPS = BASE + "wordprocessingShape"
WPG = BASE + "wordprocessingGroup"
WPC = BASE + "wordprocessingCanvas"
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
WP = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"


def shape(x, y, width, height, content, color):
    return f'''<wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{width}" cy="{height}"/></a:xfrm>
    <a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{color}"/></a:solidFill></wps:spPr>
    {content}<wps:bodyPr/></wps:wsp>'''


def drawing(ident, uri, items, width=1905000, height=825500):
    return f'''<w:r><w:drawing xmlns:wp="{WP}" xmlns:a="{A}" xmlns:wps="{WPS}" xmlns:wpg="{WPG}" xmlns:wpc="{WPC}">
    <wp:anchor simplePos="0" relativeHeight="{ident}" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1">
    <wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>190500</wp:posOffset></wp:positionH>
    <wp:positionV relativeFrom="paragraph"><wp:posOffset>25400</wp:posOffset></wp:positionV>
    <wp:extent cx="{width}" cy="{height}"/><wp:wrapSquare wrapText="bothSides"/><wp:docPr id="{ident}" name="Frame {ident}"/>
    <a:graphic><a:graphicData uri="{uri}">{items}</a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>'''


def fixture(file):
    def section(i):
        end = '<w:endnotePr><w:pos w:val="sectEnd"/></w:endnotePr>' if i == 35 else ""
        return (
            "<w:sectPr>"
            + end
            + '<w:pgSz w:w="7800" w:h="6300"/><w:pgMar w:top="600" w:bottom="600" w:left="700" w:right="700"/></w:sectPr>'
        )

    para = lambda s: f"<w:p><w:r><w:t>{s}</w:t></w:r></w:p>"
    text = "".join(f"Boxword{i:02d} " for i in range(24))
    head = shape(
        100000,
        90000,
        1200000,
        530000,
        '<wps:txbx id="18"><w:txbxContent>'
        + para(text)
        + "</w:txbxContent></wps:txbx>",
        "F0CD93",
    )
    tail = shape(0, 0, 1905000, 825500, '<wps:linkedTxbx id="18" seq="1"/>', "AFD9EE")
    rect = shape(1690000, 50000, 680000, 570000, "", "BE583C")
    group = (
        '<wpg:wgp><wpg:cNvGrpSpPr/><wpg:grpSpPr><a:xfrm rot="900000" flipH="1"><a:off x="0" y="0"/><a:ext cx="1905000" cy="825500"/><a:chOff x="0" y="0"/><a:chExt cx="1905000" cy="825500"/></a:xfrm></wpg:grpSpPr>'
        + rect
        + "</wpg:wgp>"
    )
    body = []
    notes = {
        2: (1, "First deferred note"),
        35: (2, "Second deferred note"),
        54: (3, "First document note"),
        63: (4, "Second document note"),
    }
    for i in range(70):
        body.append(para(f"Chapter {i:02d} start"))
        obs = ""
        if i in notes:
            obs = f'<w:r><w:endnoteReference w:id="{notes[i][0]}"/></w:r>'
        body.append("<w:p><w:r><w:t>Editable body</w:t></w:r>" + obs + "</w:p>")
        if i == 34:
            obs = drawing(
                8, WPC, "<wpc:wpc><wpc:bg/><wpc:whole/>" + head + rect + "</wpc:wpc>"
            )
        elif i == 35:
            obs = drawing(9, WPS, tail)
        elif i == 37:
            obs = drawing(10, WPG, group)
        else:
            obs = ""
        body.append(
            "<w:p><w:r><w:t>Anchored drawing paragraph</w:t></w:r>" + obs + "</w:p>"
        )
        pr = "<w:pPr>" + section(i) + "</w:pPr>" if i < 69 else ""
        body.append("<w:p>" + pr + "<w:r><w:t>End of chapter</w:t></w:r></w:p>")
    body.append(section(69))
    endnotes = "".join(
        f'<w:endnote w:id="{key}">' + para(label) + "</w:endnote>"
        for key, label in notes.values()
    )
    extra = {
        "word/endnotes.xml": f'<w:endnotes xmlns:w="{DOCX["W"]}">{endnotes}</w:endnotes>',
        "word/settings.xml": f'<w:settings xmlns:w="{DOCX["W"]}"><w:endnotePr><w:pos w:val="docEnd"/></w:endnotePr></w:settings>',
    }
    DOCX["package"](
        file,
        "".join(body),
        [("e", "endnotes", "endnotes.xml"), ("s", "settings", "settings.xml")],
        extra,
        {
            "word/settings.xml": "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"
        },
    )


def structure(doc):
    with ZipFile(doc) as z:
        body = ET.fromstring(z.read("word/document.xml"))
        notes = ET.fromstring(z.read("word/endnotes.xml"))
    assert len(body.findall(".//{*}endnoteReference")) == 4
    for element in ["wpc", "wgp", "linkedTxbx", "txbxContent"]:
        assert body.find(f".//{{*}}{element}") is not None, (doc, element)
    labels = [
        "".join(note.itertext()).strip()
        for note in notes
        if note.tag.endswith("}endnote")
    ]
    assert all(
        any(word in label for label in labels)
        for word in (
            "First deferred",
            "Second deferred",
            "First document",
            "Second document",
        )
    )


def printed(cli, doc, path):
    pages = HELPERS["printed_pages"](cli, doc, path)
    find = lambda text: next(i for i, page in enumerate(pages) if text in page)
    chapter35 = find("Chapter 35 start")
    end = find("Chapter 69 start")
    for label in ["First deferred note", "Second deferred note"]:
        assert find(label) == chapter35, (label, find(label), chapter35)
    for label in ["First document note", "Second document note"]:
        assert find(label) == end, (label, find(label), end)
    assert sum("Boxword" in page for page in pages) == 2
    return pages


def check_frame_offsets(old, new, out):
    section = '<w:sectPr><w:pgSz w:w="7200" w:h="5040"/><w:pgMar w:top="600" w:bottom="600" w:left="700" w:right="700"/></w:sectPr>'
    cases = [
        ("narrow-first", 260, [850000, 2095500]),
        ("wide-first", 48, [2095500, 850000]),
        ("equal", 48, [1500000]),
    ]
    results = {}
    for tag, count, widths in cases:
        story = " ".join(f"X{k:04d}" for k in range(count))
        first = f'<wps:txbx id="42"><w:txbxContent><w:p><w:r><w:t>{story}</w:t></w:r></w:p></w:txbxContent></wps:txbx>'
        body = []
        for i in range(20):
            width = widths[i % len(widths)]
            box = first if i == 0 else f'<wps:linkedTxbx id="42" seq="{i}"/>'
            item = drawing(
                i + 1, WPS, shape(0, 0, width, 760000, box, "FFFFFF"), width, 760000
            )
            body.append(f"<w:p><w:r><w:t>Frame {i}</w:t></w:r></w:p><w:p>{item}</w:p>")
            if i < 19:
                body.append("<w:p><w:pPr>" + section + "</w:pPr></w:p>")
        file = out / f"{tag}.docx"
        DOCX["package"](file, "".join(body) + section)
        signature = digest(file)
        case = {}
        for label, cli in [("baseline", old), ("candidate", new)]:
            pages = HELPERS["printed_pages"](cli, file, out / f"{tag}-{label}.pdf")
            ids = [int(m) for page in pages for m in re.findall(r"X(\d{4})", page)]
            case[label] = {
                "visible": len(ids),
                "distinct": len(set(ids)),
                "sequential": ids == list(range(len(ids))),
            }
            if label == "candidate":
                assert ids and case[label]["sequential"], (tag, ids)
                if count == 48:
                    assert len(ids) == count, (tag, ids)
                else:
                    assert 48 < len(ids) < count, (tag, ids)
        assert digest(file) == signature
        results[tag] = case
    return results


def compare_printed(first, second, old_text, new_text):
    # Only the glyphs of a corrected continuation can change on the printed page.
    # Compare all other text, including coordinates, and every pixel outside those words.
    tokens = [int(s) for page in new_text for s in re.findall(r"Boxword(\d+)", page)]
    assert tokens and tokens == list(range(len(tokens))), tokens
    assert len(old_text) == len(new_text)
    hashes = []
    changed = []
    with tempfile.TemporaryDirectory() as scratch:
        strips = []
        positions = []
        for index, pdf in enumerate([first, second]):
            prefix = Path(scratch) / f"v{index}"
            subprocess.run(
                ["pdftoppm", "-r", "48", pdf, prefix],
                capture_output=True,
                check=True,
                timeout=150,
            )
            strips.append(sorted(prefix.parent.glob(prefix.name + "-*.ppm")))
            boxes = subprocess.check_output(
                ["pdftotext", "-bbox", pdf, "-"], timeout=120
            )
            positions.append(ET.fromstring(boxes).findall(".//{*}page"))
        for i, (before, after) in enumerate(zip(*strips, strict=True)):
            hashes.append(digest(after))
            if digest(before) == hashes[-1]:
                assert old_text[i] == new_text[i]
                continue

            def words(page):
                return [
                    (
                        w.text,
                        tuple(
                            float(w.get(k)) for k in ("xMin", "yMin", "xMax", "yMax")
                        ),
                    )
                    for w in page.findall(".//{*}word")
                ]

            older, newer = [words(pages[i]) for pages in positions]
            assert len(older) == len(newer)
            masks = []
            for (left, box1), (right, box2) in zip(older, newer, strict=True):
                if left == right:
                    assert box1 == box2, (i + 1, left, box1, box2)
                else:
                    assert re.fullmatch(r"Boxword\d+", left) and re.fullmatch(
                        r"Boxword\d+", right
                    )
                    masks.extend([box1, box2])
            assert masks
            images = []
            for path in [before, after]:
                with path.open("rb") as f:
                    assert f.readline().strip() == b"P6"
                    width, height = map(int, f.readline().split())
                    assert f.readline().strip() == b"255"
                    image = bytearray(f.read())
                    assert len(image) == 3 * width * height
                    images.append(image)
            pixels = sum(
                images[0][i : i + 3] != images[1][i : i + 3]
                for i in range(0, len(images[0]), 3)
            )
            for x0, y0, x1, y1 in masks:
                left = max(0, math.floor(x0 * 48 / 72) - 1)
                right = min(width, math.ceil(x1 * 48 / 72) + 1)
                for y in range(
                    max(0, math.floor(y0 * 48 / 72) - 1),
                    min(height, math.ceil(y1 * 48 / 72) + 1),
                ):
                    span = slice(3 * (width * y + left), 3 * (width * y + right))
                    images[0][span] = images[1][span]
            assert images[0] == images[1], (
                f"page {i + 1} differs outside corrected words"
            )
            changed.append({"page": i + 1, "differentPixels": pixels})
        assert len(hashes) == len(new_text)
    return hashes, changed


def verify(old, new, desktop, out):
    original = out / "原始图形和尾注.docx"
    working = out / "分节 编辑.docx"
    fixture(original)
    structure(original)
    sha = digest(original)
    shutil.copyfile(original, working)
    prior = printed(old, original, out / "baseline.pdf")
    target = next(i + 1 for i, page in enumerate(prior) if "Chapter 34 start" in page)
    key = secrets.token_hex(32)
    number = port()
    log = out / "edit.log"
    env = {
        "JOYWORD_CONTROL_KEY": key,
        "JOYWORD_CONFIG_DIR": str(out / "profile"),
        "JOYWORD_PERF": "1",
    }
    args = ["--control", str(number), "--", str(working)]

    def inspect():
        return word_request(number, "document.inspect", key=key).get("result")

    def cmd(command, **args):
        state = inspect()
        response = word_request(
            number,
            "engine.execute",
            {
                "doc": state["doc"],
                "version": state["version"],
                "command": command,
                "params": args,
            },
            key,
        )
        assert response["ok"], response.get("error")

    def event(offset, stage):
        with log.open() as f:
            f.seek(offset)
            for line in f:
                if line.startswith("joyword-perf ") and line.endswith("\n"):
                    item = json.loads(line.removeprefix("joyword-perf "))
                    if item.get("stage") == stage:
                        return item

    addition = "Longer chapter text. " * 225
    records = []
    with app(desktop, args, out, env, log.name) as proc:
        before = wait(inspect, "drawing document", proc)
        assert len(before["summary"]["pages"]) == len(prior)
        cmd("edit.goto", page=target)
        expected = before["text"].replace(
            "Chapter 34 start", addition + "Chapter 34 start", 1
        )
        for command in ["text.insert", "edit.undo", "edit.redo"]:
            offset = log.stat().st_size
            cmd(command, **({"text": addition} if command == "text.insert" else {}))
            after = inspect()
            count = len(after["summary"]["pages"])
            assert after["text"] == (
                before["text"] if command == "edit.undo" else expected
            )
            patch = wait(lambda: event(offset, "layout_patch"), "chapter reflow", proc)
            assert patch["firstPage"] == target and patch["lastPage"] >= count, (
                target,
                patch,
                count,
            )
            records.append([command, count, patch["firstPage"], patch["lastPage"]])
        assert records[0][1] > len(prior)
        keys = word_keys(proc)
        keys("new")
        keys("enter")
        keys("enter")

        def saved():
            with ZipFile(working) as z:
                body = ET.fromstring(z.read("word/document.xml"))
            return addition.strip() in "".join(body.itertext())

        wait(saved, "saving the edited document", proc)
    with app(desktop, args, out, env, "reopen.log") as proc:
        final = wait(inspect, "fresh process", proc)
        assert final["text"] == expected and len(final["summary"]["pages"]) == count
    structure(working)
    a = printed(old, working, out / "old.pdf")
    b = printed(new, working, out / "new.pdf")
    assert len(a) == len(b) == count
    raster, changed = compare_printed(out / "old.pdf", out / "new.pdf", a, b)
    exchange = HELPERS["verify_readers"](out, old, new, count)
    for who in ("candidate", "baseline"):
        structure(out / f"{who}-resaved.docx")
    assert digest(original) == sha
    return {
        "binarySha256": digest(desktop),
        "originalSha256": sha,
        "chapters": 70,
        "initialPages": len(prior),
        "nativeEdits": records,
        "reopenedPages": count,
        "rasterizedPages": len(raster),
        "rasterSha256": raster,
        "pagesWithCorrectedText": changed,
        "crossVersion": exchange,
        "endnoteDestinations": [35, 35, 69, 69],
        "linkedFramePages": 2,
        "frameOffsets": check_frame_offsets(old, new, out),
        "originalUnchanged": True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for n in ["baseline", "candidate", "desktop", "output"]:
        parser.add_argument(n, type=Path)
    a = parser.parse_args()
    out = a.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    result = verify(
        a.baseline.resolve(), a.candidate.resolve(), a.desktop.resolve(), out
    )
    (out / "result.json").write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    )


if __name__ == "__main__":
    main()
