"""Build matched preset boards, inspect their edit geometry and check cross-version PPTX exchange."""

import argparse
import hashlib
import json
import math
import re
import runpy
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

flow = runpy.run_path(str(Path(__file__).with_name("check-flowcharts.py")))
ROOT, A = flow["ROOT"], flow["A"]
DEFINITIONS = ROOT / "crates/joydeck-geom/data/presetShapeDefinitions.xml"
SIZES = flow["SIZES"]
STATES = ("default", "asymmetric", "boundary")


def names():
    catalog = (ROOT / "crates/joydeck-geom/src/preset/catalog.rs").read_text()
    entries = re.findall(r'p!\(\s*"([^"]+)"', catalog)
    dedicated = set(
        "textBox rect roundRect snip1Rect snip2SameRect snip2DiagRect snipRoundRect round1Rect "
        "round2SameRect round2DiagRect line lineInv straightConnector1 bentConnector2 bentConnector3 "
        "curvedConnector3 rightArrow leftArrow upArrow downArrow leftRightArrow upDownArrow".split()
    )
    prefixes = (
        "flowChart",
        "actionButton",
        "callout",
        "accentCallout",
        "borderCallout",
        "accentBorderCallout",
    )
    regular = [
        name
        for name in entries
        if name not in dedicated and not name.startswith(prefixes)
    ]
    assert len(entries) == 183 and len(set(regular)) == 108
    return regular


def coordinates(shape, size, overrides):
    w, h = size
    v = dict(
        w=w, h=h, r=w, b=h, l=0, t=0, hc=w / 2, vc=h / 2, ss=min(w, h), ls=max(w, h)
    )
    for prefix, length in (("wd", w), ("hd", h), ("ssd", min(w, h))):
        for divisor in (2, 3, 4, 5, 6, 8, 10, 12, 16, 32):
            v[prefix + str(divisor)] = length / divisor
    for numerator, denom in ((1, 2), (1, 4), (1, 8), (3, 4), (3, 8), (5, 8), (7, 8)):
        v[f"{numerator if numerator > 1 else ''}cd{denom}"] = (
            21600000 * numerator / denom
        )

    def get(key):
        return v[key] if key in v else float(key)

    def evaluate(formula):
        op, *args = formula.split()
        x, y, z = ([get(a) for a in args] + [0, 0, 0])[:3]
        if op == "val":
            return x
        if op == "?:":
            return y if x > 0 else z
        if op == "pin":
            return x if y < x else min(y, z)
        if op == "*/":
            return x * y / z if z else 0
        if op == "+/":
            return (x + y) / z if z else 0
        if op == "+-":
            return x + y - z
        if op == "mod":
            return math.hypot(x, y, z)
        if op == "abs":
            return abs(x)
        if op == "sqrt":
            return math.sqrt(max(0, x))
        if op == "at2":
            return math.degrees(math.atan2(y, x)) * 60000
        if op in ("min", "max"):
            return (min if op == "min" else max)(x, y)
        if op in ("cat2", "sat2"):
            return x * (math.cos if op == "cat2" else math.sin)(math.atan2(z, y))
        if op in ("cos", "sin", "tan"):
            return x * getattr(math, op)(math.radians(y / 60000))
        raise ValueError(op)

    for section in ("avLst", "gdLst"):
        for rule in shape.findall(f"{{*}}{section}/{{*}}gd"):
            key = rule.get("name")
            value = (
                overrides[key]
                if section == "avLst" and key in overrides
                else evaluate(rule.get("fmla"))
            )
            v[key] = max(-1e15, min(value, 1e15))
    return get


def controls(shape, get):
    for handle in shape.findall("{*}ahLst/*"):
        p = handle.find("{*}pos")
        for axis in ("X", "Y", "R", "Ang"):
            ref = handle.get("gdRef" + axis)
            if ref is not None:
                yield (
                    ref,
                    axis,
                    [get(handle.get(k + axis)) for k in ("min", "max")],
                    [get(p.get(k)) for k in ("x", "y")],
                )


def adjustments(shape, size, state):
    result = {
        g.get("name"): int(g.get("fmla").split()[1])
        for g in shape.findall("{*}avLst/{*}gd")
    }
    if state == "default":
        return {}
    handles = list(controls(shape, coordinates(shape, size, result)))
    if not handles:
        return {}
    index = list(SIZES.values()).index(size) % len(handles)
    key, axis, (lo, hi), _ = handles[index]
    if axis != "Ang":
        # Unlimited leader tips can be placed on neighboring tiles. Use the
        # edge of this tile for those controls and real domain limits elsewhere.
        lo, hi = max(lo, -46000), min(hi, 110000)
    fraction = (
        (0.34 if index % 2 == 0 else 0.67)
        if state == "asymmetric"
        else (0.0 if index % 2 == 0 else 1.0)
    )
    result[key] = round(lo + (hi - lo) * fraction)
    return result


def generate(out):
    full = names()
    assert flow["sha"](DEFINITIONS.read_bytes()) == flow["SPEC_SHA"]
    spec = ET.parse(DEFINITIONS).getroot()
    metrics = []
    with tempfile.TemporaryDirectory() as temp:
        for first in range(0, len(full), 27):
            group = full[first : first + 27]
            for state in STATES:
                pages = []
                for aspect, size in SIZES.items():
                    part = Path(temp) / f"{first}-{state}-{aspect}.pptx"
                    flow["board"](part, group, size, False)
                    with ZipFile(part) as archive:
                        slide = ET.fromstring(archive.read("ppt/slides/slide1.xml"))
                    for item, name in zip(
                        slide.findall(".//{*}spTree/{*}sp"), group, strict=True
                    ):
                        definition = spec.find(name)
                        overrides = adjustments(definition, size, state)
                        guides = item.find("{*}spPr/{*}prstGeom/{*}avLst")
                        for key, v in overrides.items():
                            ET.SubElement(
                                guides, f"{{{A}}}gd", name=key, fmla=f"val {v}"
                            )
                        # Geometry functions receive points, not EMUs.
                        get = coordinates(
                            definition, [d / 12700 for d in size], overrides
                        )
                        rect = definition.find("{*}rect")
                        frame = (
                            [get(k) for k in ("l", "t", "r", "b")]
                            if rect is None
                            else [get(rect.get(k)) for k in ("l", "t", "r", "b")]
                        )
                        sites = [
                            [get(point.get(k)) for k in ("x", "y")]
                            for point in definition.findall("{*}cxnLst/{*}cxn/{*}pos")
                        ]
                        values = [
                            v for k, v in overrides.items() if k.startswith("adj")
                        ]
                        metrics.append(
                            dict(
                                name=name,
                                state=state,
                                size=[n / 12700 for n in size],
                                adjustments=values,
                                frame=frame,
                                sites=sites,
                            )
                        )
                    pages.append(ET.tostring(slide, encoding="utf-8"))
                dest = out / f"JoyOffice-presets-{first // 27 + 1}-{state}.pptx"
                with ZipFile(part) as seed, ZipFile(dest, "x", ZIP_DEFLATED) as archive:
                    for info in seed.infolist():
                        m = re.fullmatch(r"ppt/slides/slide([123])\.xml", info.filename)
                        archive.writestr(
                            info, pages[int(m[1]) - 1] if m else seed.read(info)
                        )

    def row(item):
        def numbers(values):
            return ",".join(format(v, ".10g") for v in values) or "-"

        return " ".join(
            (
                item["name"],
                numbers(item["size"]),
                numbers(item["adjustments"]),
                numbers(item["frame"]),
                numbers([v for site in item["sites"] for v in site]),
            )
        )

    by_shape = {name: [] for name in full}
    for item in metrics:
        by_shape[item["name"]].append(row(item))
    assert all(len(rows) == 9 for rows in by_shape.values())
    (out / "geometry.txt").write_text("\n".join(row(item) for item in metrics) + "\n")
    (out / "gallery-geometry.txt").write_text(
        "\n".join(rows[index % 9] for index, rows in enumerate(by_shape.values()))
        + "\n"
    )
    (out / "gallery.json").write_text(json.dumps(full) + "\n")


def slide_semantics(deck):
    spec = ET.parse(DEFINITIONS).getroot()
    with ZipFile(deck) as archive:
        result = []
        for i in (1, 2, 3):
            page = ET.fromstring(archive.read(f"ppt/slides/slide{i}.xml"))
            shapes = []
            for obj in page.findall(".//{*}spTree/{*}sp"):
                g = obj.find("{*}spPr/{*}prstGeom")
                props = obj.find("{*}nvSpPr/{*}cNvPr")
                guides = {
                    k.get("name"): k.get("fmla") for k in g.findall("{*}avLst/{*}gd")
                }
                for fixed in spec.find(g.get("prst")).findall("{*}avLst/{*}gd"):
                    key = fixed.get("name")
                    if not key.startswith("adj"):
                        guides.setdefault(key, fixed.get("fmla"))
                shapes.append((props.get("name"), g.get("prst"), guides))
            result.append(shapes)
    return result


def render(cli, deck, dest):
    flow["support"]["run"](cli, "render", deck, "--output", dest, "--scale", 1, "--all")
    files = sorted(dest.glob("*.png"))
    assert len(files) == 3
    return [hashlib.sha256(f.read_bytes()).hexdigest() for f in files]


def textbox(desktop, cli, old, scene, out):
    sys.path.insert(0, str(ROOT / "scripts"))
    from native_pilot_support import DeckControl, app, wait

    out.mkdir()
    opts = {
        "JOYDECK_CONFIG_DIR": str(out / "profile"),
        "JOYDECK_RECOVERY_DIR": str(out / "recovery"),
    }
    label = "JoyOffice 图文路径 123"
    saved = out / "文本框.pptx"

    def args(p):
        return [str(p), "--window-size=1100x740", "--control-stdio=" + str(out)]

    with app(desktop, args(scene), out, opts) as proc:
        c = DeckControl(proc)

        def shapes():
            return c.inspect()["slides"][0]["objects"]

        wait(c.inspect, "opened", proc)
        initial = len(shapes())
        c.edit({"command": {"command": "insert-text", "x": 44, "y": 63, "text": label}})
        assert len(shapes()) == initial + 1
        for action, total in (("undo", initial), ("redo", initial + 1)):
            c.edit({"command": {"command": action}})
            assert len(shapes()) == total
        assert not c.request("document.save", params={"path": str(saved)}, **c.guard())[
            "dirty"
        ]
    with app(desktop, args(saved), out, opts, "reopen.log") as proc:
        c = DeckControl(proc)
        snap = wait(c.inspect, "text box reopened", proc)
        assert len(snap["slides"][0]["objects"]) == initial + 1
    for path in (saved, out / "regenerated.pptx"):
        if path != saved:
            flow["support"]["run"](
                cli, "exchange", saved, "--mode", "regenerate", "--output", path
            )
        with ZipFile(path) as archive:
            page = ET.fromstring(archive.read("ppt/slides/slide1.xml"))
            (box,) = [
                s
                for s in page.findall(".//{*}spTree/{*}sp")
                if label in [n.text for n in s.findall(".//{*}t")]
            ]
            assert box.find("{*}nvSpPr/{*}cNvSpPr").get("txBox") in ("1", "true")
            assert box.find("{*}spPr/{*}prstGeom").get("prst") == "rect"
        for binary in (cli, old):
            snap = json.loads(flow["support"]["run"](binary, "inspect", path).stdout)
            assert label in json.dumps(snap, ensure_ascii=False)
    return {
        "rect": True,
        "txBox": True,
        "text": label,
        "undoRedo": True,
        "desktopReopen": True,
        "twoVersionExchange": True,
    }


def verify(inputs, out, old, cli, desktop):
    report = {}
    for deck in sorted(inputs.glob("JoyOffice-presets-*.pptx")):
        stage = out / deck.stem
        stage.mkdir()
        second = stage / "regenerated.pptx"
        run = flow["support"]["run"]
        run(cli, "exchange", deck, "--mode", "regenerate", "--output", second)
        assert slide_semantics(deck) == slide_semantics(second), deck
        result = {}
        for key, binary in (("baseline", old), ("candidate", cli)):
            hashes = render(binary, deck, stage / key)
            assert hashes == render(binary, second, stage / (key + "-reopened")), (
                key,
                deck,
            )
            result[key] = hashes
        report[deck.stem] = result
    assert len(report) == 12
    if desktop:
        source = inputs / "JoyOffice-presets-1-asymmetric.pptx"
        report["resize"] = flow["native"](
            desktop, source, out / "native", 27, "triangle"
        )
        report["textBox"] = textbox(desktop, cli, old, source, out / "text-box")
    (out / "report.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)
    make = sub.add_parser("generate")
    make.add_argument("output", type=Path)
    check = sub.add_parser("verify")
    for name in ("inputs", "output", "baseline", "candidate"):
        check.add_argument(name, type=Path)
    check.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    if args.cmd == "generate":
        generate(args.output.resolve())
    else:
        verify(
            args.inputs.resolve(),
            args.output.resolve(),
            args.baseline.resolve(),
            args.candidate.resolve(),
            args.desktop and args.desktop.resolve(),
        )
