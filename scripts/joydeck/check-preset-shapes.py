"""Compare independent DrawingML shapes through rendering, native PPTX writing and old-version reading."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import struct
import shutil
import xml.etree.ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[2]
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
P = "http://schemas.openxmlformats.org/presentationml/2006/main"
KINDS = ["roundRect", "rightArrow", "leftArrow", "downArrow", "upArrow", "leftRightArrow", "upDownArrow"]
RECTANGLES = ["rect", "roundRect", "snip1Rect", "snip2SameRect", "snip2DiagRect",
              "snipRoundRect", "round1Rect", "round2SameRect", "round2DiagRect"]
GALLERY_CONNECTORS = ["line", "lineInv", "straightConnector1", "bentConnector2", "bentConnector3", "curvedConnector3"]
FILE_CONNECTORS = ["curvedConnector2", "bentConnector4", "curvedConnector4", "bentConnector5", "curvedConnector5"]
ADJUST = [None, 0, 50_000, 100_000]
COLOR = (30, 112, 206)


def fixture(dest, kinds, adjustments, rectangles, connectors=False):
    with ZipFile(ROOT / "tests/fixtures/joydeck/independent.pptx") as src, ZipFile(dest, "x", ZIP_DEFLATED) as new:
        for name in src.namelist():
            data = src.read(name)
            if name == "ppt/slides/slide1.xml":
                slide = ET.fromstring(data)
                tree = slide.find(".//{*}spTree")
                for old in list(tree)[2:]:
                    tree.remove(old)
                stride_x, stride_y = (970000, 950000) if rectangles or connectors else (1280000, 1220000)
                width, height = (820000, 770000) if rectangles or connectors else (1100000, 920000)
                for col, kind in enumerate(kinds):
                    for row, adjust in enumerate(adjustments):
                        idx = 2 + col * len(adjustments) + row
                        av = ""
                        if adjust is not None and kind != "rect":
                            names = ["adj"] if kind in ("roundRect", "snip1Rect", "round1Rect") else ["adj1", "adj2"]
                            values = adjust if isinstance(adjust, tuple) else (adjust, adjust)
                            av = "".join(f'<a:gd name="{n}" fmla="val {v}"/>' for n, v in zip(names, values))
                        if connectors:
                            guides = max(0, int(kind[-1]) - 2) if "Connector" in kind else 0
                            values = adjust if isinstance(adjust, tuple) else (adjust,) * guides
                            av = "" if adjust is None else "".join(
                                f'<a:gd name="adj{i + 1}" fmla="val {v}"/>' for i, v in enumerate(values[:guides]))
                        style = ('<a:noFill/><a:ln w="38100"><a:solidFill><a:srgbClr val="1E70CE"/>'
                                 '</a:solidFill></a:ln>') if connectors else (
                                 '<a:solidFill><a:srgbClr val="1E70CE"/></a:solidFill><a:ln><a:noFill/></a:ln>')
                        x, y = 160000 + col * stride_x, 300000 + row * stride_y
                        # Exercise the text rectangle on old documents as well as its filled outline.
                        label = ('<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="900"/>'
                                 '<a:t>Test</a:t></a:r></a:p></p:txBody>') if rectangles else ""
                        tag, nv, details = ("cxnSp", "nvCxnSpPr", "cNvCxnSpPr") if connectors else ("sp", "nvSpPr", "cNvSpPr")
                        tree.append(ET.fromstring(f"""
                        <p:{tag} xmlns:p="{P}" xmlns:a="{A}"><p:{nv}><p:cNvPr id="{idx}" name="{kind}-{row}"/>
                        <p:{details}/><p:nvPr/></p:{nv}><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/>
                        <a:ext cx="{width}" cy="{height}"/></a:xfrm><a:prstGeom prst="{kind}"><a:avLst>{av}</a:avLst>
                        </a:prstGeom>{style}
                        </p:spPr>{label}</p:{tag}>"""))
                data = ET.tostring(slide, encoding="utf-8")
            new.writestr(name, data)


def presets(path):
    with ZipFile(path) as archive:
        slide = ET.fromstring(archive.read("ppt/slides/slide1.xml"))
    return {shape.find(".//{*}cNvPr").get("name"):
            [geo.get("prst"), {g.get("name"): g.get("fmla") for g in geo.findall(".//{*}gd")}]
            for shape in slide.findall(".//{*}spTree/*") if (geo := shape.find(".//{*}prstGeom")) is not None}


def make_connector_reference(source, output, definitions):
    # Render the unmodified ECMA guide equations as custom geometry in the previous CLI,
    # independent of both versions' preset generators.
    spec = ET.parse(definitions).getroot()
    with ZipFile(source) as src, ZipFile(output, "x", ZIP_DEFLATED) as dst:
        for name in src.namelist():
            data = src.read(name)
            if name == "ppt/slides/slide1.xml":
                slide = ET.fromstring(data)
                for props in slide.findall(".//{*}cxnSp/{*}spPr"):
                    preset = props.find("{*}prstGeom")
                    ref = spec.find(preset.get("prst"))
                    assert ref is not None
                    custom = ET.Element(f"{{{A}}}custGeom")
                    for part in ref:
                        custom.append(copy.deepcopy(part))
                    overrides = {g.get("name"): g.get("fmla") for g in preset.findall(".//{*}gd")}
                    for guide in custom.findall("{*}avLst/{*}gd"):
                        if guide.get("name") in overrides:
                            guide.set("fmla", overrides.pop(guide.get("name")))
                    assert not overrides
                    for path in custom.findall("{*}pathLst/{*}path"):
                        path.set("fill", "none")
                    position = list(props).index(preset)
                    props.remove(preset)
                    props.insert(position, custom)
                data = ET.tostring(slide, encoding="utf-8")
            dst.writestr(name, data)


def run(cli, *args):
    return subprocess.run([str(cli), *map(str, args)], check=True, capture_output=True, timeout=150)


def render(cli, path, out):
    run(cli, "render", path, "--output", out, "--page", 1, "--scale", 1)
    png = next(out.glob("*.png"))
    with png.open("rb") as f:
        header = f.read(24)
    width, height = struct.unpack(">2I", header[16:24])
    try:
        from PIL import Image
    except ImportError:
        converter = shutil.which("magick") or shutil.which("convert")
        if not converter:
            raise RuntimeError("Pillow or ImageMagick is needed for RGB decoding") from None
        pixels = subprocess.check_output([converter, str(png), "-alpha", "remove", "-depth", "8", "rgb:-"])
    else:
        with Image.open(png) as image:
            pixels = image.convert("RGB").tobytes()
    assert len(pixels) == width * height * 3
    assert sum(rgb == COLOR for rgb in struct.iter_unpack("3B", pixels)) > 1000
    return (width, height), pixels


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("baseline", "candidate", "output"):
        parser.add_argument(arg, type=Path)
    parser.add_argument("--family", choices=["arrows", "rectangles", "connectors", "file-connectors"], default="arrows")
    parser.add_argument("--definitions", type=Path, help="ECMA-376 Part 1 presetShapeDefinitions.xml for connector reference images")
    args = parser.parse_args()
    rectangles = args.family == "rectangles"
    connectors = args.family in ("connectors", "file-connectors")
    if connectors and args.definitions is None:
        parser.error("connector families require --definitions")
    kinds = {"arrows": KINDS, "rectangles": RECTANGLES, "connectors": GALLERY_CONNECTORS,
             "file-connectors": FILE_CONNECTORS}[args.family]
    adjustments = [*ADJUST, (20_000, 75_000, 35_000)] if connectors else (
        [*ADJUST, (20_000, 35_000)] if rectangles else ADJUST)
    out = args.output.resolve()
    out.mkdir(exist_ok=False, parents=True)
    src, generation = out / "presets.pptx", out / "regenerated.pptx"
    fixture(src, kinds, adjustments, rectangles, connectors)
    before = presets(src)
    assert len(before) == len(kinds) * len(adjustments)
    run(args.candidate, "exchange", src, "--mode", "regenerate", "--output", generation)
    after = presets(generation)
    if before != after:
        raise ValueError(f"lost preset data: {[(k, v, after.get(k)) for k, v in before.items() if v != after.get(k)]}")
    images = [render(cli, path, out / folder) for cli, path, folder in
              [(args.baseline, src, "old"), (args.candidate, src, "new"),
               (args.candidate, generation, "reopen"), (args.baseline, generation, "rollback")]]
    assert all(size == images[0][0] for size, _ in images)
    encoded = [raw for _, raw in images]
    if encoded[1] != encoded[2] or encoded[0] != encoded[3]:
        raise ValueError("reopened shape pixels differ from original on the same reader")
    different = sum(a != b for a, b in zip(struct.iter_unpack("3B", encoded[0]), struct.iter_unpack("3B", encoded[1])))
    if different and args.family != "file-connectors":
        raise ValueError(f"{different} pixels differ from the previous renderer")
    report = {"shapeInstances":len(before), "presets":kinds, "adjustments":adjustments, "pptxParametersPreserved":True,
              "imageSize":images[0][0], "changedPixelsFromPreviousRenderer":different,
              "rgbSha256":[hashlib.sha256(raw).hexdigest() for raw in encoded],
              "bothVersionsReopenedRegeneratedFile":True}
    if connectors:
        reference = out / "ecma-reference.pptx"
        make_connector_reference(src, reference, args.definitions)
        _, expected = render(args.baseline, reference, out / "standard")
        reference_diff = sum(a != b for a, b in zip(struct.iter_unpack("3B", expected),
                                                     struct.iter_unpack("3B", encoded[1])))
        if reference_diff:
            raise ValueError(f"{reference_diff} pixels differ from the independent DrawingML guide rendering")
        report.update(presentationObject="p:cxnSp", referenceRgbSha256=hashlib.sha256(expected).hexdigest(),
                      changedPixelsFromPublishedGuides=reference_diff,
                      definitionsSha256=hashlib.sha256(args.definitions.read_bytes()).hexdigest())
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
