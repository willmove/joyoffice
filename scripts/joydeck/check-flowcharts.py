"""Compare all DrawingML flowcharts in old and new Deck builds, then resize one in the native editor."""

import argparse
import copy
from functools import lru_cache
import hashlib
import json
import math
from pathlib import Path
import runpy
import struct
import sys
import xml.etree.ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED

support = runpy.run_path(str(Path(__file__).with_name("check-preset-shapes.py")))
A, P = support["A"], support["P"]
ROOT = support["ROOT"]
SPEC_SHA = "2f7c868d857c1e3c4b5a6068759fe0e07d77ad58377a6618d1b02ba3507b6939"
SIZES = {"wide": (1700000, 680000), "tall": (600000, 1140000), "square": (1000000, 1000000)}


def origin(index, size):
    return (
        130000 + index % 6 * 1990000 + (1780000 - size[0]) // 2,
        130000 + index // 6 * 1320000 + (1140000 - size[1]) // 2,
    )


def sha(data):
    return hashlib.sha256(data).hexdigest()


def board(dest, names, size, labels):
    seed = ROOT / "tests/fixtures/joydeck/independent.pptx"
    with ZipFile(seed) as src, ZipFile(dest, "x", ZIP_DEFLATED) as out:
        for part in src.namelist():
            data = src.read(part)
            if part == "ppt/slides/slide1.xml":
                doc = ET.fromstring(data)
                tree = doc.find(".//{*}spTree")
                for old in list(tree)[2:]:
                    tree.remove(old)
                for i, name in enumerate(names):
                    # Center each preset in a fixed tile so the same inputs cover
                    # square, landscape and portrait shapes on one slide.
                    x, y = origin(i, size)
                    label = (
                        (
                            '<p:txBody><a:bodyPr lIns="0" rIns="0" tIns="0" bIns="0" anchor="ctr"/>'
                            '<a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr sz="750"/>'
                            "<a:t>Go</a:t></a:r></a:p></p:txBody>"
                        )
                        if labels
                        else ""
                    )
                    tree.append(
                        ET.fromstring(f'''<p:sp xmlns:p="{P}" xmlns:a="{A}">
<p:nvSpPr><p:cNvPr id="{i + 2}" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{size[0]}" cy="{size[1]}"/></a:xfrm>
<a:prstGeom prst="{name}"><a:avLst/></a:prstGeom>
<a:solidFill><a:srgbClr val="1E70CE"/></a:solidFill>
<a:ln w="19050"><a:solidFill><a:srgbClr val="17212E"/></a:solidFill></a:ln></p:spPr>{label}</p:sp>''')
                    )
                data = ET.tostring(doc, encoding="utf-8")
            out.writestr(part, data)


def differences(lhs, rhs):
    assert len(lhs) == len(rhs)
    return sum(a != b for a, b in zip(struct.iter_unpack("3B", lhs), struct.iter_unpack("3B", rhs)))


def text_frame(shape, w, h):
    values = dict(w=w, h=h, l=0.0, t=0.0, r=w, b=h, hc=w / 2, vc=h / 2)
    for name, distance in (("wd", w), ("hd", h), ("ssd", min(w, h))):
        for divisor in (2, 3, 4, 5, 6, 8, 10):
            values[name + str(divisor)] = distance / divisor

    def value(token):
        return float(token) if token.lstrip("-").isdigit() else values[token]

    for guide in shape.findall("{*}gdLst/{*}gd"):
        op, *args = guide.get("fmla").split()
        v = list(map(value, args))
        if op == "*/":
            result = v[0] * v[1] / v[2]
        elif op == "+-":
            result = v[0] + v[1] - v[2]
        elif op == "at2":
            result = 60000 * math.degrees(math.atan2(v[1], v[0]))
        elif op in ("cos", "sin"):
            result = v[0] * getattr(math, op)(math.radians(v[1] / 60000))
        else:
            raise ValueError(f"unsupported standard guide {op}")
        values[guide.get("name")] = result
    rectangle = shape.find("{*}rect")
    return tuple(value(rectangle.get(a)) for a in ("l", "t", "r", "b"))


def put_reference_labels(custom, dest, definitions):
    # The previous version discards a:custGeom/a:rect. Put the reference
    # text into the rectangle specified by the attachment in page coordinates.
    with ZipFile(custom) as src, ZipFile(dest, "x", ZIP_DEFLATED) as out:
        for part in src.namelist():
            data = src.read(part)
            if part == "ppt/slides/slide1.xml":
                doc = ET.fromstring(data)
                tree = doc.find(".//{*}spTree")
                for i, shape in enumerate(tree.findall("{*}sp")):
                    body = shape.find("{*}txBody")
                    if body is None:
                        continue
                    label = copy.deepcopy(shape)
                    shape.remove(body)
                    name = label.find("{*}nvSpPr/{*}cNvPr")
                    kind = name.get("name")
                    name.set("name", kind + "-reference-text")
                    name.set("id", str(1000 + i))
                    label.find("{*}nvSpPr/{*}cNvSpPr").set("txBox", "1")
                    prop = label.find("{*}spPr")
                    frame = prop.find("{*}xfrm")
                    off, ext = frame.find("{*}off"), frame.find("{*}ext")
                    l, t, r, b = text_frame(
                        definitions.find(kind), int(ext.get("cx")), int(ext.get("cy"))
                    )
                    for key, p, shift in (("x", off, l), ("y", off, t)):
                        p.set(key, str(round(int(p.get(key)) + shift)))
                    ext.set("cx", str(round(r - l)))
                    ext.set("cy", str(round(b - t)))
                    prop.clear()
                    prop.append(frame)
                    prop.append(
                        ET.fromstring(f'<a:prstGeom xmlns:a="{A}" prst="rect"><a:avLst/></a:prstGeom>')
                    )
                    prop.append(ET.Element(f"{{{A}}}noFill"))
                    line = ET.SubElement(prop, f"{{{A}}}ln")
                    ET.SubElement(line, f"{{{A}}}noFill")
                    tree.append(label)
                data = ET.tostring(doc, encoding="utf-8")
            out.writestr(part, data)


@lru_cache(maxsize=4096)
def paint(rgb):
    # Solve the red/blue channel mixture of white (255,255,255), fill
    # (30,112,206) and ink (23,33,46). A gray stroke on white must remain
    # gray even when its RGB midpoint is closer to blue than to either end.
    red, _, blue = rgb
    fill = (232 * (blue - 46) - 209 * (red - 23)) / (232 * 160 - 209 * 7)
    background = (red - 23 - 7 * fill) / 232
    amounts = (background, fill, 1 - background - fill)
    return amounts.index(max(amounts))


def contour_differences(lhs, rhs, width, height):
    rgb_outliers = paint_outliers = 0
    for p, (a, b) in enumerate(zip(struct.iter_unpack("3B", lhs), struct.iter_unpack("3B", rhs))):
        if a == b:
            continue
        x, y = p % width, p // width
        neighbors = [
            (yy * width + xx) * 3
            for yy in range(max(0, y - 1), min(y + 2, height))
            for xx in range(max(0, x - 1), min(x + 2, width))
        ]
        if max(abs(u - v) for u, v in zip(a, b)) > 2:
            if any(
                not min(other[q + c] for q in neighbors) - 2
                <= v
                <= max(other[q + c] for q in neighbors) + 2
                for sample, other in ((a, rhs), (b, lhs))
                for c, v in enumerate(sample)
            ):
                rgb_outliers += 1
        if paint(a) != paint(b):
            if any(
                all(paint(tuple(other[q : q + 3])) != paint(sample) for q in neighbors)
                for sample, other in ((a, rhs), (b, lhs))
            ):
                paint_outliers += 1
    return rgb_outliers, paint_outliers


def reference_hits(pixels, dims, names, size):
    width, height = dims
    rows = []
    for index, name in enumerate(names):
        x, y = origin(index, size)
        flags = []
        for iy in range(9):
            for ix in range(9):
                px = int((x + (ix + 0.5) / 9 * size[0]) * width / 12191999)
                py = int((y + (iy + 0.5) / 9 * size[1]) * height / 6858000)
                colors = {
                    pixels[(v * width + u) * 3 : (v * width + u) * 3 + 3]
                    for u in range(px - 1, px + 2)
                    for v in range(py - 1, py + 2)
                }
                flags.append(
                    "1"
                    if colors == {bytes((30, 112, 206))}
                    else "0"
                    if colors == {bytes((255, 255, 255))}
                    else "."
                )
        rows.append(f"{name} {size[0]} {size[1]} {''.join(flags)}")
    return rows


def native(binary, scene, out):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from native_pilot_support import DeckControl, app, wait

    out.mkdir()
    source_hash = sha(scene.read_bytes())
    config = {"JOYDECK_CONFIG_DIR": str(out / "profile"), "JOYDECK_RECOVERY_DIR": str(out / "recovery")}
    saved = out / "流程图 修改.pptx"

    def launch(path):
        return [str(path), "--window-size=1100x740", "--control-stdio=" + str(out)]

    def objects(snapshot):
        return snapshot["slides"][0]["objects"]

    with app(binary, launch(scene), out, config) as process:
        client = DeckControl(process)
        first = wait(client.inspect, "flowchart slide opened", process)
        shapes = objects(first)
        assert len(shapes) == 29
        target = next(s for s in shapes if s.get("name") == "flowChartDocument")

        def selected():
            return next(s["transform"] for s in objects(client.inspect()) if s["id"] == target["id"])

        before = selected()
        client.edit(
            {
                "command": {
                    "command": "at",
                    "page": 1,
                    "shapes": [target["id"]],
                    "edit": {"command": "transform", "dx": 5.0, "dy": 3.0, "dw": 17.0, "dh": 9.0},
                }
            }
        )
        after = selected()
        for key, delta in (("x", 5.0), ("y", 3.0), ("w", 17.0), ("h", 9.0)):
            assert abs(after[key] - before[key] - delta) < 0.001, (key, before, after)
        for action, expected in (("undo", before), ("redo", after)):
            client.edit({"command": {"command": action}})
            assert selected() == expected
        result = client.request("document.save", params={"path": str(saved)}, **client.guard())
        assert not result["dirty"] and sha(scene.read_bytes()) == source_hash
    with app(binary, launch(saved), out, config, "reopened.log") as process:
        client = DeckControl(process)
        reopened = objects(wait(client.inspect, "resized shape reopened", process))
        shape = next(s for s in reopened if s["name"] == "flowChartDocument")
        assert len(reopened) == 29 and all(
            abs(shape["transform"][k] - after[k]) < 0.001 for k in ("x", "y", "w", "h")
        )
    with ZipFile(saved) as z:
        slide = ET.fromstring(z.read("ppt/slides/slide1.xml"))
        assert len(slide.findall(".//{*}prstGeom")) == 29
    return {
        "binarySha256": sha(binary.read_bytes()),
        "shapes": len(reopened),
        "resized": "flowChartDocument",
        "before": before,
        "after": after,
        "undoRedo": True,
        "savedAndReopened": True,
        "sourceUnchanged": True,
    }


def verify(old, new, definitions, out, desktop):
    assert sha(definitions.read_bytes()) == SPEC_SHA
    spec = ET.parse(definitions).getroot()
    names = [node.tag for node in spec if node.tag.startswith("flowChart")]
    assert len(names) == 29
    report = {
        "definitionsSha256": SPEC_SHA,
        "shapes": names,
        "renderers": [sha(c.read_bytes()) for c in (old, new)],
        "boards": {},
    }
    hits = []
    run = support["run"]
    render = support["render"]
    presets = support["presets"]
    for kind, size in SIZES.items():
        for labels in (False, True):
            name = kind + ("-text" if labels else "-outline")
            directory = out / name
            directory.mkdir()
            src, ref, second = (directory / (k + ".pptx") for k in ("preset", "custom", "regenerated"))
            board(src, names, size, labels)
            support["make_geometry_reference"](src, ref, definitions, False)
            if labels:
                labelled = directory / "custom-text.pptx"
                put_reference_labels(ref, labelled, spec)
                ref = labelled
            run(new, "exchange", src, "--mode", "regenerate", "--output", second)
            assert presets(src) == presets(second)
            (width, height), expected = render(old, ref, directory / "reference")
            if not labels:
                hits.extend(reference_hits(expected, (width, height), names, size))
            results = {}
            for label, cli, deck in [
                ("old", old, src),
                ("new", new, src),
                ("oldReopened", old, second),
                ("newReopened", new, second),
            ]:
                dims, pixels = render(cli, deck, directory / label)
                assert dims == (width, height)
                results[label] = pixels
            assert results["old"] == results["oldReopened"] and results["new"] == results["newReopened"]
            color_outliers, paint_outliers = contour_differences(
                expected, results["new"], width, height
            )
            assert paint_outliers == 0, (name, paint_outliers)
            report["boards"][name] = {
                "size": [width, height],
                "shapes": 29,
                "rgbSha256": {k: sha(v) for k, v in results.items()},
                "referenceSha256": sha(expected),
                "referenceDifferentPixels": differences(expected, results["new"]),
                "referencePixelsBeyondRgbNeighborhood": color_outliers,
                "referencePaintPixelsBeyondOnePixel": paint_outliers,
                "oldDifferentPixels": differences(results["old"], results["new"]),
            }
    (out / "reference-hits.txt").write_text("\n".join(hits) + "\n")
    if desktop:
        report["native"] = native(desktop, out / "wide-text/preset.pptx", out / "desktop")
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("baseline", "candidate", "definitions", "output"):
        parser.add_argument(key, type=Path)
    parser.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    verify(
        args.baseline.resolve(),
        args.candidate.resolve(),
        args.definitions.resolve(),
        output,
        args.desktop and args.desktop.resolve(),
    )
