"""Render a known WMF and EMF through Word and Deck and compare same-host baseline pixels."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
RGB = (19, 177, 97)


def pack(fmt, *values):
    return struct.pack("<" + fmt, *values)


def polygon(kind):
    """An asymmetric triangle exercises signed coordinate pairs and a solid brush."""
    color = int.from_bytes(bytes(RGB), "little")
    pts = (4, 7, 97, 23, 27, 92)
    if kind == "wmf":
        def record(code, body):
            return pack("IH", 3 + len(body) // 2, code) + body
        records = [record(0x02FC, pack("HIH", 0, color, 0)), record(0x012D, pack("H", 0)),
                   record(0x0324, pack("H6h", 3, *pts)), record(0, b"")]
        body = b"".join(records)
        return pack("HHHIHIH", 1, 9, 0x300, 9 + len(body) // 2, 1, max(map(len, records)) // 2, 0) + body

    def record(code, body):
        return pack("II", code, 8 + len(body)) + body
    bounds = (0, 0, 100, 100)
    body = b"".join([record(39, pack("IIII", 1, 0, color, 0)), record(37, pack("I", 1)),
                     record(3, pack("4iI6i", *bounds, 3, *pts)), record(14, pack("III", 0, 0, 20))])
    header = pack("II4i4iIIIIHHIII4i", 1, 88, *bounds, 0, 0, 2500, 2500, 0x464D4520, 0x10000,
                  88 + len(body), 5, 2, 0, 0, 0, 0, 1000, 500, 250, 125)
    return header + body


def document(path, kind):
    word = path.suffix == ".docx"
    source = "assets/samples/mixed-media.docx" if word else "tests/fixtures/joydeck/independent.pptx"
    old = "blocks.png" if word else "image1.png"
    new = old[:-3] + kind
    with zipfile.ZipFile(ROOT / source) as inp, zipfile.ZipFile(path, "x", zipfile.ZIP_DEFLATED) as out:
        for name in inp.namelist():
            data = inp.read(name)
            if name.endswith("/" + old):
                name, data = name[:-3] + kind, polygon(kind)
            elif name == "[Content_Types].xml":
                data = data.replace(b'</Types>', f'<Default Extension="{kind}" ContentType="image/x-{kind}"/></Types>'.encode())
            elif name.endswith(".rels"):
                data = data.replace(("/" + old).encode(), ("/" + new).encode())
            elif name == "word/document.xml":
                xml = ET.fromstring(data)
                body = xml.find("{*}body")
                for child in list(body):
                    if child.find(".//{*}drawing") is None and not child.tag.endswith("}sectPr"):
                        body.remove(child)
                data = ET.tostring(xml, encoding="utf-8")
            out.writestr(name, data)


def pixels(path):
    try:
        from PIL import Image
    except ImportError:
        converter = shutil.which("magick") or shutil.which("convert")
        if not converter:
            raise RuntimeError("Install Pillow or ImageMagick to compare rendered pixels") from None
        data = subprocess.check_output([converter, str(path), "-alpha", "remove", "-depth", "8", "rgb:-"])
    else:
        with Image.open(path) as image:
            data = image.convert("RGB").tobytes()
    count = sum(pixel == RGB for pixel in struct.iter_unpack("BBB", data))
    if count < 10:
        raise ValueError(f"{path.name}: triangle colour missing ({count} matching pixels)")
    return {"rgbSha256": hashlib.sha256(data).hexdigest(), "trianglePixels": count}


def render(bin_dir, out):
    samples = {}
    for ext, product, page in [("docx", "word", 1), ("pptx", "deck", 2)]:
        binary = str(bin_dir / ("joy" + product + "-cli"))
        for kind in ("wmf", "emf"):
            prefix = f"{product}-{kind}"
            path, pngs, pdf = out / f"{kind}.{ext}", out / (prefix + "-png"), out / (prefix + ".pdf")
            document(path, kind)
            common = [binary, "render", str(path), "--output", str(pngs), "--page", str(page)]
            if product == "word":
                font = str(ROOT / "assets/fonts/SourceSerif4-Regular.ttf") + ":0"
                png_args = common + ["--font", font, "--dpi", "96"]
                pdf_args = [binary, "convert", str(path), "--to", "pdf", "--font", font, "--output", str(pdf)]
            else:
                png_args = common
                pdf_args = [binary, "export", str(path), "--format", "pdf", "--pages", str(page), "--output", str(pdf)]
            for cmd in [png_args, pdf_args, ["pdftoppm", "-r", "96", "-f", "1", "-l", "1", "-singlefile", str(pdf), str(out / prefix)]]:
                subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, timeout=90)
            png, = pngs.glob("*.png")
            samples[prefix] = {"screenScene": pixels(png), "pdfRaster": pixels(out / (prefix + ".ppm"))}
    return samples


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("binary_dir", type=Path)
    p.add_argument("output", type=Path)
    p.add_argument("--baseline", type=Path)
    args = p.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    samples = render(args.binary_dir.resolve(), out)
    (out / "pixels.json").write_text(json.dumps(samples, indent=2) + "\n")
    if args.baseline and samples != json.loads(args.baseline.read_text()):
        raise SystemExit("Rendered screen scene or PDF pixels differ from the baseline")


if __name__ == "__main__":
    main()
