"""Render two combined GDI fixtures through the frozen and current JoyDeck CLIs."""

import argparse
import hashlib
import json
from pathlib import Path
from struct import pack, pack_into
import subprocess
from zipfile import ZIP_DEFLATED, ZipFile

from PIL import Image


def digest(data):
    return hashlib.sha256(data).hexdigest()


def wmf():
    def record(opcode, payload=b""):
        payload += bytes(len(payload) % 2)
        return pack("<IH", 3 + len(payload) // 2, opcode) + payload

    polygon = pack("<H8h", 4, 0, 0, 100, 0, 100, 100, 0, 100)
    label = b"WMF clip text"
    text = pack("<hhHH4h", 37, 15, len(label), 4, 14, 35, 87, 72)
    text += label + bytes(len(label) % 2) + pack("<" + "h" * len(label), *([6] * len(label)))
    records = [
        record(0x0103, pack("<H", 8)),
        record(0x020C, pack("<hh", 100, 100)),
        record(0x020E, pack("<hh", 100, 100)),
        record(0x020B, pack("<hh", 1, 2)),
        record(0x020D, pack("<hh", 1, 2)),
        record(0x02FC, pack("<HIH", 0, 0x000000DD, 0)),
        record(0x012D, pack("<H", 0)),
        record(0x001E),
        record(0x0416, pack("<4h", 88, 87, 12, 13)),
        record(0x0324, polygon),
        record(0x0209, pack("<I", 0x00EE0000)),
        record(0x0A32, text),
        record(0x0127, pack("<h", -1)),
        record(0x0418, pack("<4h", 98, 15, 89, 2)),
        record(0),
    ]
    body = b"".join(records)
    header = pack("<HHHIHIH", 1, 9, 0x300, (18 + len(body)) // 2, 1, max(map(len, records)) // 2, 0)
    return header + body


def emf():
    def record(opcode, payload=b""):
        payload += bytes(-len(payload) % 4)
        return pack("<II", opcode, 8 + len(payload)) + payload

    label = "EMF clip text"
    encoded = label.encode("utf-16-le")
    text = pack("<4iIff2i3I4iI", 0, 0, 100, 100, 1, 1, 1, 14, 40, len(label),
                76, 4 | 0x2000, 12, 38, 88, 70, 76 + len(encoded))
    text += encoded + pack("<" + "ii" * len(label), *([6, 0] * len(label)))
    poly = pack("<4iI8h", 0, 0, 100, 100, 4, 0, 0, 0, 100, 100, 100, 100, 0)
    records = [
        record(39, pack("<IIII", 1, 0, 0x000000DD, 0)),
        record(37, pack("<I", 1)),
        record(33),
        record(35, pack("<6f", 0.9, 0.05, -0.05, 0.9, 5, 4)),
        record(30, pack("<4i", 11, 12, 91, 89)),
        record(86, poly),
        record(24, pack("<I", 0x00EE0000)),
        record(84, text),
        record(34, pack("<i", -1)),
        record(42, pack("<4i", 2, 89, 15, 98)),
        record(14, pack("<3I", 0, 0, 20)),
    ]
    body = b"".join(records)
    header = bytearray(88)
    pack_into("<2I8i4I2H", header, 0, 1, 88, 0, 0, 100, 100, 0, 0, 2500, 2500,
              0x464D4520, 0x10000, 88 + len(body), 1 + len(records), 2, 0)
    pack_into("<4i", header, 72, 1000, 500, 250, 125)
    return bytes(header) + body


def fixture(source, destination, member, payload):
    with ZipFile(source) as original, ZipFile(destination, "w", ZIP_DEFLATED) as output:
        assert original.namelist().count(member) == 1, member
        for name in original.namelist():
            output.writestr(name, payload if name == member else original.read(name))


def render(cli, source, output):
    subprocess.run([str(cli.resolve()), "render", str(source.resolve()), "--page", "2",
                    "--output", str(output.resolve())], check=True, capture_output=True, timeout=120)
    image_path, = output.glob("*.png")
    with Image.open(image_path) as image:
        rgb = image.convert("RGB")
        histogram = rgb.getcolors(rgb.width * rgb.height)
        red = sum(n for n, (r, g, b) in histogram if r > 160 and g < 30 and b < 30)
        blue = sum(n for n, (r, g, b) in histogram if b > 160 and r < 30 and g < 30)
        assert red > 100 and blue > 10, (source, red, blue)
        return {"size": list(rgb.size), "sha256": digest(rgb.tobytes()),
                "redShapePixels": red, "blueTextPixels": blue}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("shells", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    checks = {}
    for kind, payload in (("wmf", wmf()), ("emf", emf())):
        source = args.output / f"{kind}.pptx"
        fixture(args.shells / f"{kind}.pptx", source, f"ppt/media/image1.{kind}", payload)
        before = render(args.frozen, source, args.output / f"{kind}-frozen")
        after = render(args.candidate, source, args.output / f"{kind}-candidate")
        assert before == after, (kind, before, after)
        checks[kind] = {"metafileSha256": digest(payload), "raster": after}
    (args.output / "report.json").write_text(json.dumps(checks, indent=2) + "\n",
                                            encoding="utf-8", newline="\n")
    print("Two vector, device, clipping and text scenes match the frozen build pixel for pixel")


if __name__ == "__main__":
    main()
