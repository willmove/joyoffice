"""Compare named DrawingML geometry and an independently calculated literal path in Word and Deck."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import runpy
import secrets
import shutil
import struct
import subprocess
import xml.etree.ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED

from native_pilot_support import DeckControl, app, digest, port, wait, word_request
from verify_save_changes import word_keys

ROOT = Path(__file__).resolve().parents[1]
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
WPS = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
WP = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
MC = "http://schemas.openxmlformats.org/markup-compatibility/2006"
MARK = "urn:joyoffice:test:custom-geometry"
BLUE = (36, 112, 198)
FORMULAS = [
    ("val wd10", 10),
    ("*/ 3 8 2", 12),
    ("+- 5 9 3", 11),
    ("+/ 16 8 2", 12),
    ("?: -1 30 13", 13),
    ("abs -9", 9),
    ("at2 0 1", 5400000),
    ("cat2 20 3 4", 12),
    ("sat2 20 3 4", 16),
    ("cos 12 0", 12),
    ("sin 13 cd4", 13),
    ("tan 15 cd8", 15),
    ("max 8 16", 16),
    ("min 8 16", 8),
    ("mod 4 12 3", 13),
    ("pin 5 60 14", 14),
    ("sqrt 121", 11),
]


def geom(named):
    # 100 million path units keep rounding the reference cubic controls to integer EMUs
    # far below one display pixel. Each tile has an asymmetric, signed quarter-ellipse.
    width = 100_000_000
    rules, paths = [], []
    for i in range(64):
        x = (i % 8) * 12_000_000 + 5_500_000
        y = (i // 8) * 12_000_000 + 5_500_000
        formula, expected = FORMULAS[i % len(FORMULAS)]
        if formula == "val wd10":
            formula = "*/ wd10 1 1000000"
        if formula.startswith("at2 "):
            rules.append(f'<a:gd name="angle{i}" fmla="{formula}"/>')
            formula, expected = f"*/ angle{i} 1 450000", 12
        rx, ry = 2_400_000, 1_600_000
        start, sign = i % 4, -1 if i % 3 == 0 else 1
        rad = start * math.pi / 2
        after = rad + sign * math.pi / 2

        def point(t):
            return round(x + rx * math.cos(t)), round(y + ry * math.sin(t))

        p0, p1 = point(rad), point(after)
        k = (4 / 3) * math.tan(sign * math.pi / 8)
        c1 = (
            round(p0[0] - k * rx * math.sin(rad)),
            round(p0[1] + k * ry * math.cos(rad)),
        )
        c2 = (
            round(p1[0] + k * rx * math.sin(after)),
            round(p1[1] - k * ry * math.cos(after)),
        )
        cx = x - 3_500_000 + expected * 60_000

        def pt(xy):
            return f'<a:pt x="{xy[0]}" y="{xy[1]}"/>'

        def vertex(tag, xy):
            return f"<a:{tag}>{pt(xy)}</a:{tag}>"

        if named:
            for key, expr in (
                (f"selector{i}", formula),
                (f"offset{i}", f"*/ selector{i} 60000 1"),
                (f"cx{i}", f"+- {x - 3500000} offset{i} 0"),
                (f"cy{i}", f"val {y + 3200000}"),
                (f"leadx{i}", f"val {p0[0]}"),
                (f"leady{i}", f"val {p0[1]}"),
            ):
                rules.append(f'<a:gd name="{key}" fmla="{expr}"/>')
            line = vertex("moveTo", (f"cx{i}", f"cy{i}")) + vertex(
                "lnTo", (f"leadx{i}", f"leady{i}")
            )
            line += f'<a:arcTo wR="rx" hR="ry" stAng="{start * 5400000}" swAng="{sign * 5400000}"/>'
        else:
            line = vertex("moveTo", (cx, y + 3200000)) + vertex("lnTo", p0)
            line += (
                "<a:cubicBezTo>"
                + "".join(pt(p) for p in (c1, c2, p1))
                + "</a:cubicBezTo>"
            )
        paths.append(f'<a:path w="{width}" h="{width}">{line}<a:close/></a:path>')
    extras = (
        f'<a:extLst><a:ext uri="{MARK}"><j:formula j:token="j:source">external &amp; retained</j:formula></a:ext></a:extLst>'
        if named
        else ""
    )
    return (
        '<a:custGeom><a:avLst><a:gd name="rx" fmla="val 2400000"/><a:gd name="ry" fmla="val 1600000"/></a:avLst>'
        + "<a:gdLst>"
        + "".join(rules)
        + '</a:gdLst><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/>'
        + "<a:pathLst>"
        + "".join(paths)
        + "</a:pathLst>"
        + extras
        + "</a:custGeom>"
    )


def fixture(path, named):
    geometry = geom(named)
    style = (
        '<a:solidFill><a:srgbClr val="2470C6"/></a:solidFill><a:ln><a:noFill/></a:ln>'
    )
    w, h = 5200000, 3450000
    if path.suffix == ".docx":
        package = runpy.run_path(str(ROOT / "scripts/joyword/check-complex-corpus.py"))[
            "package"
        ]
        frame = f'<a:xfrm><a:off x="0" y="0"/><a:ext cx="{w}" cy="{h}"/></a:xfrm>'
        drawing = f'<w:drawing xmlns:a="{A}" xmlns:wp="{WP}" xmlns:wps="{WPS}" xmlns:j="{MARK}" xmlns:mc="{MC}" mc:Ignorable="j"><wp:inline><wp:extent cx="{w}" cy="{h}"/><wp:docPr id="7" name="Shared curves"/><a:graphic><a:graphicData uri="{WPS}"><wps:wsp><wps:cNvSpPr/><wps:spPr>{frame}{geometry}{style}</wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:inline></w:drawing>'
        package(
            path,
            "<w:p><w:r><w:t>Original</w:t></w:r></w:p><w:p><w:r>"
            + drawing
            + "</w:r></w:p><w:sectPr/>",
        )
    else:
        source = ROOT / "tests/fixtures/joydeck/independent.pptx"
        with ZipFile(source) as inp, ZipFile(path, "x", ZIP_DEFLATED) as out:
            for name in inp.namelist():
                data = inp.read(name)
                if name == "ppt/slides/slide1.xml":
                    left, last = (
                        data.decode().split("</p:grpSpPr>", 1)
                        if b"</p:grpSpPr>" in data
                        else data.decode().split("<p:grpSpPr/>", 1)
                    )
                    split = (
                        "</p:grpSpPr>" if b"</p:grpSpPr>" in data else "<p:grpSpPr/>"
                    )
                    right = last[last.index("</p:spTree>") :]
                    shape = f'<p:sp xmlns:j="{MARK}"><p:nvSpPr><p:cNvPr id="7" name="Shared curves"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="600000" y="400000"/><a:ext cx="{w}" cy="{h}"/></a:xfrm>{geometry}{style}</p:spPr></p:sp>'
                    data = (left + split + shape + right).encode()
                out.writestr(inp.getinfo(name), data)


def compare_xml(a, b):
    def shape(path):
        part = (
            "word/document.xml" if path.suffix == ".docx" else "ppt/slides/slide1.xml"
        )
        with ZipFile(path) as z:
            return ET.fromstring(z.read(part)).find(".//{" + A + "}custGeom")

    def tree(e):
        return [
            e.tag,
            sorted((k, v) for k, v in e.attrib.items() if k != "{" + MC + "}Ignorable"),
            (e.text or "").strip(),
            list(map(tree, e)),
        ]

    before, after = shape(a), shape(b)
    assert tree(before) == tree(after)
    if a.suffix == ".docx":
        assert "j" in after.get("{" + MC + "}Ignorable", "").split()
    assert (
        len(after.findall(".//{*}arcTo")) == 64
        and after.find(".//{" + MARK + "}formula") is not None
    )
    return digest(b)


def rgb(binary, source, out, pdf=False):
    word = source.suffix == ".docx"
    font = str(ROOT / "assets/fonts/SourceSerif4-Regular.ttf") + ":0"
    if pdf:
        dest = out.with_suffix(".pdf")
        args = (
            ["convert", source, "--to", "pdf", "--font", font]
            if word
            else ["export", source, "--format", "pdf", "--pages", "1"]
        )
        subprocess.run(
            [binary, *args, "--output", dest], check=True, stdout=subprocess.DEVNULL
        )
        subprocess.run(
            ["pdftoppm", "-f", "1", "-l", "1", "-r", "96", "-singlefile", dest, out],
            check=True,
        )
        image = out.with_suffix(".ppm")
    else:
        args = ["render", source, "--output", out, "--page", "1"]
        if word:
            args += ["--font", font, "--dpi", "96"]
        subprocess.run([binary, *args], check=True, stdout=subprocess.DEVNULL)
        (image,) = out.glob("*.png")
    try:
        from PIL import Image

        with Image.open(image) as im:
            return im.convert("RGB").tobytes()
    except ImportError:
        return subprocess.check_output(
            [
                shutil.which("magick") or "convert",
                image,
                "-alpha",
                "remove",
                "-depth",
                "8",
                "rgb:-",
            ]
        )


def checked_refusals(cli, source, out):
    cases = {
        "mandatory-extension": (b'mc:Ignorable="j"', b"", "read-only"),
        "linked-extension": (
            b"<j:formula ",
            b'<j:formula r:id="unresolved" ',
            "relationship",
        ),
    }
    reasons = {}
    for label, (before, after, expected) in cases.items():
        changed, dest = out / (label + ".docx"), out / (label + "-saved.docx")
        with ZipFile(source) as archive, ZipFile(changed, "x", ZIP_DEFLATED) as write:
            for entry in archive.infolist():
                body = archive.read(entry)
                if entry.filename == "word/document.xml":
                    assert before in body
                    body = body.replace(before, after, 1)
                write.writestr(entry, body)
        sha = digest(changed)
        process = subprocess.run(
            [
                cli,
                "run",
                changed,
                "--cmd",
                'text.insert={"text":"Checked"}',
                "--save",
                dest,
            ],
            capture_output=True,
            text=True,
        )
        reason = (process.stdout + process.stderr).strip()
        assert (
            process.returncode
            and expected in reason.lower()
            and not dest.exists()
            and digest(changed) == sha
        ), reason
        reasons[label] = reason
    return reasons


def desktop_word(binary, source, out):
    working = out / "Word 共享曲线.docx"
    shutil.copyfile(source, working)
    key, number = secrets.token_hex(32), port()
    config = {
        "JOYWORD_CONTROL_KEY": key,
        "JOYWORD_CONFIG_DIR": str(out / "word-profile"),
        "JOYWORD_PERF": "1",
    }
    launch = ["--control", str(number), "--", str(working)]

    def inspect():
        return word_request(number, "document.inspect", key=key).get("result")

    with app(binary, launch, out, config, "word.log") as proc:
        first = wait(inspect, "Word opens curves", proc)
        text = first["text"]
        for action in ("text.insert", "edit.undo", "edit.redo"):
            state = inspect()
            params = {"text": "Checked "} if action == "text.insert" else {}
            reply = word_request(
                number,
                "engine.execute",
                {
                    "doc": state["doc"],
                    "version": state["version"],
                    "command": action,
                    "params": params,
                },
                key,
            )
            assert reply["ok"] and inspect()["text"] == (
                text if action == "edit.undo" else "Checked " + text
            )
        send = word_keys(proc)
        for keypress in ("new", "enter", "enter"):
            send(keypress)

        def written():
            with ZipFile(working) as z:
                return b"Checked " in z.read("word/document.xml")

        wait(written, "Word commits edited copy", proc)
    with app(binary, launch, out, config, "word-reopened.log") as proc:
        assert wait(inspect, "Word reopens curves", proc)["text"] == "Checked " + text
    return working


def desktop_deck(binary, source, out):
    saved = out / "Deck 共享曲线.pptx"
    config = {
        "JOYDECK_CONFIG_DIR": str(out / "deck-profile"),
        "JOYDECK_RECOVERY_DIR": str(out / "deck-recovery"),
    }

    def launch(path):
        return [str(path), "--window-size=1100x740", "--control-stdio=" + str(out)]

    def obj(client):
        return client.inspect()["slides"][0]["objects"][0]

    with app(binary, launch(source), out, config, "deck.log") as proc:
        client = DeckControl(proc)
        shape = wait(lambda: obj(client), "Deck opens curves", proc)
        before = shape["transform"]
        client.edit(
            {
                "command": {
                    "command": "at",
                    "page": 1,
                    "shapes": [shape["id"]],
                    "edit": {
                        "command": "transform",
                        "dx": 5.0,
                        "dy": 3.0,
                        "dw": 17.0,
                        "dh": 9.0,
                    },
                }
            }
        )
        after = obj(client)["transform"]
        for name, delta in (("x", 5), ("y", 3), ("w", 17), ("h", 9)):
            assert abs(after[name] - before[name] - delta) < 0.001
        for action, expected in (("undo", before), ("redo", after)):
            client.edit({"command": {"command": action}})
            assert obj(client)["transform"] == expected
        client.request("document.save", params={"path": str(saved)}, **client.guard())
    with app(binary, launch(saved), out, config, "deck-reopened.log") as proc:
        client = DeckControl(proc)
        reopened = wait(lambda: obj(client), "Deck reopens curves", proc)["transform"]
        assert all(abs(after[k] - reopened[k]) < 0.001 for k in ("x", "y", "w", "h"))
    return saved


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "out"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    result = {}
    for name, ext, desktop in (
        ("word", "docx", desktop_word),
        ("deck", "pptx", desktop_deck),
    ):
        old, new = (
            root.resolve() / f"joy{name}-cli"
            for root in (args.baseline, args.candidate)
        )
        src, ref = (out / (f"{name}-{kind}.{ext}") for kind in ("named", "literal"))
        fixture(src, True)
        fixture(ref, False)
        before = digest(src)
        cases = {}
        for pdf, tag in ((False, "screen"), (True, "pdf")):
            control = rgb(old, ref, out / f"{name}-literal-{tag}", pdf)
            actual = rgb(new, src, out / f"{name}-candidate-{tag}", pdf)
            assert len(actual) == len(control)
            diffs = [abs(a - b) for a, b in zip(actual, control) if a != b]
            counts = sum(p == BLUE for p in struct.iter_unpack("3B", actual))
            legacy = rgb(old, src, out / f"{name}-baseline-{tag}", pdf)
            cases[tag] = {
                "bytes": len(actual),
                "bluePixels": counts,
                "differingChannels": len(diffs),
                "maxChannelDelta": max(diffs, default=0),
                "rgbSha256": hashlib.sha256(actual).hexdigest(),
                "baselineNamedDifferingChannels": sum(
                    a != b for a, b in zip(legacy, control)
                ),
            }
            print(name, tag, cases[tag], flush=True)
            assert counts > 500 and len(diffs) < 128 and max(diffs, default=0) <= 8
        record = {
            "sourceSha256": before,
            "oldCliSha256": digest(old),
            "cliSha256": digest(new),
            "paths": 64,
            "guideOperations": 17,
            "render": cases,
        }
        if name == "word":
            record["refusals"] = checked_refusals(new, src, out)
        if args.native:
            executable = args.candidate.resolve() / ("joy" + name)
            saved = desktop(executable, src, out)
            record["native"] = {
                "savedSha256": compare_xml(src, saved),
                "desktopSha256": digest(executable),
                "undoRedoReopen": True,
                "externalGeometryPreserved": True,
            }
            for cli in (old, new):
                subprocess.run(
                    [cli, "inspect", saved], stdout=subprocess.DEVNULL, check=True
                )
        assert digest(src) == before
        result[name] = record
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
