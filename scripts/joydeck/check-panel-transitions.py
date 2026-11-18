"""Measure the two slide textures in PowerPoint and JoyDeck tile, slat and ripple transitions."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import runpy
import subprocess
import tempfile
from xml.etree import ElementTree as ET
from zipfile import ZIP_DEFLATED, ZipFile

MASKS = runpy.run_path(str(Path(__file__).with_name("check-soft-masks.py")))
MOTION = MASKS["motion"]
CASES = ([("none", "")] + [(k, o) for k in ("checker", "blinds", "comb") for o in ("horz", "vert")]
         + [("ripple", o) for o in ("center", "lu", "ru", "ld", "rd")] + [("checker", "horz")])


PRISMS = [("none", "")] + [(kind, direction) for kind in ("cube", "box", "rotate", "orbit") for direction in "lrud"]


PRESETS = [("none", "")] + [(kind, opt) for kind, options in MOTION["window"]["PRESET_OPTIONS"].items() for opt in options]
FABRIC_PRESETS = {"fallOver", "drape", "curtains"}


def marked(path, cases=CASES):
    """Add neutral strokes to the two colored pages to measure their motion during transitions."""
    p, a = MOTION["P"], MOTION["window"]["A"]
    with tempfile.TemporaryDirectory() as tmp:
        seed = Path(tmp) / "plain.pptx"
        MOTION["generate"](seed, cases)
        with ZipFile(seed) as src, ZipFile(path, "x", ZIP_DEFLATED) as dest:
            size = ET.fromstring(src.read("ppt/presentation.xml")).find("{*}sldSz")
            width, height = (int(size.get(k)) for k in ("cx", "cy"))
            for name in src.namelist():
                data = src.read(name)
                slide = re.fullmatch(r"ppt/slides/slide(\d+)\.xml", name)
                if slide:
                    root = ET.fromstring(data)
                    tree = root.find(".//{*}spTree")
                    offset = 0.06 if int(slide[1]) % 2 == 0 else 0.
                    boxes = [(0.16 + offset, 0.11, 0.06, 0.76, "F0F0F0"),
                             (0.45, 0.25 + offset, 0.49, 0.08, "F0F0F0"),
                             (0.72 - offset, 0.45, 0.08, 0.41, "202020"),
                             (0.26, 0.69 - offset, 0.34, 0.08, "202020")]
                    for i, (x,y,w,h,color) in enumerate(boxes, 100):
                        shape = ET.SubElement(tree, f"{{{p}}}sp")
                        props = ET.SubElement(shape, f"{{{p}}}nvSpPr")
                        ET.SubElement(props, f"{{{p}}}cNvPr", id=str(i), name=f"Marker {i}")
                        ET.SubElement(props, f"{{{p}}}cNvSpPr")
                        ET.SubElement(props, f"{{{p}}}nvPr")
                        style = ET.SubElement(shape, f"{{{p}}}spPr")
                        xf = ET.SubElement(style, f"{{{a}}}xfrm")
                        ET.SubElement(xf, f"{{{a}}}off", x=str(round(x*width)), y=str(round(y*height)))
                        ET.SubElement(xf, f"{{{a}}}ext", cx=str(round(w*width)), cy=str(round(h*height)))
                        geo = ET.SubElement(style, f"{{{a}}}prstGeom", prst="rect")
                        ET.SubElement(geo, f"{{{a}}}avLst")
                        fill = ET.SubElement(style, f"{{{a}}}solidFill")
                        ET.SubElement(fill, f"{{{a}}}srgbClr", val=color)
                        line = ET.SubElement(style, f"{{{a}}}ln")
                        ET.SubElement(line, f"{{{a}}}noFill")
                    data = ET.tostring(root, encoding="utf-8", xml_declaration=True)
                dest.writestr(name, data)


def weights(image, index, colors):
    # Red and blue are linearly independent. Measure both texture contributions
    # so a shaded or edge-on face cannot pass as an ordinary crossfade.
    old, new = colors[(index - 1) % 2], colors[index % 2]
    det = old[0] * new[2] - new[0] * old[2]
    amounts = []
    for pixel in MASKS["SAMPLES"]:
        red, _, blue = image[pixel * 3:pixel * 3 + 3]
        amounts.extend((max(0., min(1., (new[2] * red - new[0] * blue) / det)),
                        max(0., min(1., (old[0] * blue - old[2] * red) / det))))
    return amounts


def samples(image, index, colors, markers):
    if markers:
        return [v / 255 for i in MASKS["SAMPLES"] for v in image[3 * i:3 * i + 3]]
    return weights(image, index, colors)


def measure(movie, deck, raw, markers, cases=CASES):
    pixels = raw.read_bytes() if raw else subprocess.check_output([
        "ffmpeg", "-v", "error", "-i", movie, "-vf", "scale=96:54:flags=area", "-vsync", "0",
        "-pix_fmt", "rgb24", "-f", "rawvideo", "-",
    ])
    stride = 96 * 54 * 3
    frame_count = len(cases) * 74 - 50
    assert len(pixels) == frame_count * stride

    def frame(n):
        return pixels[n * stride:(n + 1) * stride]

    colors = [frame(i)[:3] for i in (8, 80)]
    observations = []
    for index, (kind, option) in enumerate(cases[1:], 1):
        for j in range(1, 16):
            sample = samples(frame(25 + (index - 1) * 74 + 3 * j), index, colors, markers)
            observations.append([index + 1, kind, option, j / 16,
                                 bytes(round(x * 255) for x in sample).hex()])
    return {"source": "PowerPoint 16.115 for macOS, 1920x1080, 30 fps, 1600 ms effects",
            "movieSha256": hashlib.sha256(movie.read_bytes()).hexdigest(), "frames": frame_count,
            "family": "presets2013" if cases == PRESETS else "prisms" if cases == PRISMS else "panels",
            "payloadSha256": MASKS["payloads"](deck), "markers": markers, "pixels": MASKS["SAMPLES"], "samples": observations}


def exchange_effects(original, old, new, out, originals, cases):
    versions = {"baseline": old, "candidate": new}
    middle = [[i + 2, kind, direction, 0.5] for i, (kind, direction) in enumerate(cases[1:])]
    checked = {}
    for first, second in (("baseline", "candidate"), ("candidate", "baseline")):
        source = original
        for generation, writer in enumerate((first, second), 1):
            tag = f"{first}-{generation}"
            saved = out / f"{tag}.pptx"
            subprocess.run([versions[writer], "exchange", source, "--mode", "regenerate", "--output", saved],
                           check=True, capture_output=True)
            with ZipFile(saved) as archive:
                for page, kind, direction, _ in middle:
                    tree = ET.fromstring(archive.read(f"ppt/slides/slide{page}.xml"))
                    turn = tree.find(".//{*}transition")
                    assert turn is not None and turn.get(f"{{{MOTION['P14']}}}dur") == "1600"
                    xml_tag, attrs = MOTION["window"]["effect_xml"](kind, direction)
                    effect = turn.find(xml_tag)
                    assert effect is not None, (page, xml_tag)
                    for key, value in attrs.items():
                        if key in {"isContent", "isInverted", "invX"}:
                            assert (effect.get(key, "0") in ("true", "1")) == (value == "1")
                        else:
                            assert effect.get(key) == value
            replay = out / f"{tag}-held.pptx"
            MOTION["hold_for_sampling"](saved, replay, len(cases))
            checked[tag] = {}
            for reader, exe in versions.items():
                images = MOTION["sample_cli"](exe, replay, middle, out / f"{tag}-{reader}-frames")
                manifest = [hashlib.sha256(image).hexdigest() for image in images]
                assert manifest == originals[reader], (tag, reader)
                checked[tag][reader] = hashlib.sha256("".join(manifest).encode()).hexdigest()
            source = saved
    return checked


def verify(reference, old, new, out):
    out.mkdir(parents=True)
    prisms = reference.get("family") == "prisms"
    presets = reference.get("family") == "presets2013"
    cases = PRESETS if presets else PRISMS if prisms else CASES
    original = out / ("预设转场.pptx" if presets else "立体翻转.pptx" if prisms else "棋盘百叶梳形波纹.pptx")
    replay = out / "sample.pptx"
    markers = reference.get("markers", False)
    if markers:
        marked(original, cases)
    else:
        MOTION["generate"](original, cases)
    assert MASKS["payloads"](original) == reference["payloadSha256"]
    MOTION["hold_for_sampling"](original, replay, len(cases))
    rows = reference["samples"]
    report = {"binaries": [hashlib.sha256(b.read_bytes()).hexdigest() for b in (old, new)]}
    middle = {}
    for label, cli in (("baseline", old), ("candidate", new)):
        frames = MOTION["sample_cli"](cli, replay, rows, out / label)
        if prisms or presets:
            middle[label] = [hashlib.sha256(frame).hexdigest() for row, frame in zip(rows, frames, strict=True) if row[3] == 0.5]
        errors = {}
        for (page, kind, option, _, target), frame in zip(rows, frames, strict=True):
            actual = samples(frame, page - 1, MASKS["SOURCE"], markers)
            errors.setdefault(kind + "/" + option, []).extend(
                abs(a - b / 255) for a, b in zip(actual, bytes.fromhex(target), strict=True))
        report[label] = {k: MASKS["summarize"](v) for k,v in errors.items()}
    if prisms or presets:
        report["exchange"] = exchange_effects(original, old, new, out, middle, cases)
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    for option, stats in report["candidate"].items():
        if presets and option.split("/")[0] not in FABRIC_PRESETS:
            assert stats == report["baseline"][option], (option, stats)
        elif prisms and not markers and option.split("/")[0] in ("rotate", "orbit"):
            # There are no foreground shapes on the plain page. Only the two
            # backgrounds can contribute, through their original cross-fade.
            assert stats == report["baseline"][option] and stats["meanAbsoluteError"] < 0.035, (option, stats)
        else:
            assert stats["meanAbsoluteError"] < report["baseline"][option]["meanAbsoluteError"], (option, stats)


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    modes = p.add_subparsers(dest="mode", required=True)
    for mode, arguments in (("generate", ["deck"]), ("measure", ["movie", "deck", "output"]),
                            ("verify", ["reference", "baseline", "candidate", "output"])):
        sub = modes.add_parser(mode)
        for name in arguments:
            sub.add_argument(name, type=Path)
        if mode != "verify":
            sub.add_argument("--markers", action="store_true")
            family = sub.add_mutually_exclusive_group()
            family.add_argument("--presets", action="store_true", help="all 20 directions and variants of p15:prstTrans")
            family.add_argument("--prisms", action="store_true", help="all four directions of the four p14:prism effects")
        if mode == "measure":
            sub.add_argument("--raw", type=Path)
    a = p.parse_args()
    cases = PRESETS if getattr(a, "presets", False) else PRISMS if getattr(a, "prisms", False) else CASES
    if a.mode == "generate":
        if a.markers:
            marked(a.deck, cases)
        else:
            MOTION["generate"](a.deck, cases)
    elif a.mode == "measure":
        a.output.write_text(json.dumps(measure(a.movie, a.deck, a.raw, a.markers, cases), indent=2) + "\n")
    else:
        verify(json.loads(a.reference.read_text()), a.baseline.resolve(), a.candidate.resolve(), a.output.resolve())
