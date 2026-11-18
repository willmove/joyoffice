"""Compare independent DrawingML shapes through rendering, native PPTX writing and old-version reading."""
import argparse
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
ADJUST = [None, 0, 50_000, 100_000]
COLOR = (30, 112, 206)


def fixture(dest):
    with ZipFile(ROOT / "tests/fixtures/joydeck/independent.pptx") as src, ZipFile(dest, "x", ZIP_DEFLATED) as new:
        for name in src.namelist():
            data = src.read(name)
            if name == "ppt/slides/slide1.xml":
                slide = ET.fromstring(data)
                tree = slide.find(".//{*}spTree")
                for old in list(tree)[2:]:
                    tree.remove(old)
                for col, kind in enumerate(KINDS):
                    for row, adjust in enumerate(ADJUST):
                        idx = 2 + col * len(ADJUST) + row
                        av = ""
                        if adjust is not None:
                            names = ["adj"] if kind == "roundRect" else ["adj1", "adj2"]
                            av = "".join(f'<a:gd name="{n}" fmla="val {adjust}"/>' for n in names)
                        x, y = 160000 + col * 1280000, 300000 + row * 1220000
                        tree.append(ET.fromstring(f"""
                        <p:sp xmlns:p="{P}" xmlns:a="{A}"><p:nvSpPr><p:cNvPr id="{idx}" name="{kind}-{row}"/>
                        <p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/>
                        <a:ext cx="1100000" cy="920000"/></a:xfrm><a:prstGeom prst="{kind}"><a:avLst>{av}</a:avLst>
                        </a:prstGeom><a:solidFill><a:srgbClr val="1E70CE"/></a:solidFill><a:ln><a:noFill/></a:ln>
                        </p:spPr></p:sp>"""))
                data = ET.tostring(slide, encoding="utf-8")
            new.writestr(name, data)


def presets(path):
    with ZipFile(path) as archive:
        slide = ET.fromstring(archive.read("ppt/slides/slide1.xml"))
    return {shape.find(".//{*}cNvPr").get("name"):
            [geo.get("prst"), {g.get("name"): g.get("fmla") for g in geo.findall(".//{*}gd")}]
            for shape in slide.findall(".//{*}sp") if (geo := shape.find(".//{*}prstGeom")) is not None}


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
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(exist_ok=False, parents=True)
    src, generation = out / "presets.pptx", out / "regenerated.pptx"
    fixture(src)
    before = presets(src)
    assert len(before) == len(KINDS) * len(ADJUST)
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
    report = {"shapeInstances":len(before), "presets":KINDS, "adjustments":ADJUST, "pptxParametersPreserved":True,
              "imageSize":images[0][0], "changedPixelsFromPreviousRenderer":different,
              "rgbSha256":[hashlib.sha256(raw).hexdigest() for raw in encoded],
              "bothVersionsReopenedRegeneratedFile":True}
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
