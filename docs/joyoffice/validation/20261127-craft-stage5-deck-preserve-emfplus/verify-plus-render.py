"""Compare metafile clipping, path shapes, images, and code-page text against stage 0."""

import argparse
import importlib.util
import json
from pathlib import Path
from struct import pack, pack_into

from PIL import Image

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-xml-vector/verify-metafiles.py"
SPEC = importlib.util.spec_from_file_location("metafile_fixture", PREVIOUS)
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)


def emfplus():
    def plus(opcode, flags, payload=b""):
        pad = bytes(-len(payload) % 4)
        return pack("<HHII", opcode, flags, 12 + len(payload) + len(pad), len(payload)) + payload + pad

    def emf(opcode, payload):
        payload += bytes(-len(payload) % 4)
        return pack("<II", opcode, 8 + len(payload)) + payload

    def comment(*records):
        body = b"".join(records)
        return emf(70, pack("<II", len(body) + 4, 0x2B464D45) + body)

    label = "Żółć · Ёж"
    family = "Arial"
    font = pack("<If4I", 0xDBC01002, 11, 0, 0, 0, len(family)) + family.encode("utf-16-le")
    diamond = pack("<3I8f4B", 0xDBC01002, 4, 0, 5, 50, 50, 5, 95, 50, 50, 95, 0, 1, 1, 0x81)
    pixels = pack("<7I4I", 0xDBC01002, 1, 2, 2, 8, 0x0026200A, 0,
                  0xFF118855, 0xFFEEEE44, 0xFF118855, 0xFFEEEE44)
    first = comment(
        plus(0x4001, 0, pack("<4I", 0xDBC01002, 0, 96, 96)),
        plus(0x4008, 0x0600, font),
        plus(0x4008, 0x0309, diamond),
        plus(0x4025, 0, pack("<I", 5)),
        plus(0x4033, 9),
        plus(0x400A, 0x8000, pack("<II4f", 0xFFDD0000, 1, 0, 0, 100, 100)),
        plus(0x4008, 0x8504, pack("<I", len(pixels)) + pixels[:20]),
    )
    second = comment(
        plus(0x4008, 0x0504, pixels[20:]),
        plus(0x401A, 4, pack("<II8f", 0, 2, 0, 0, 2, 2, 38, 57, 24, 18)),
        plus(0x401C, 0x8000, pack("<III4f", 0xFF0000EE, 0xffffffff, len(label), 27, 37, 54, 19)
             + label.encode("utf-16-le")),
        plus(0x4026, 0, pack("<I", 5)),
    )
    body = first + second + emf(14, pack("<3I", 0, 0, 20))
    header = bytearray(88)
    pack_into("<2I8i4I2H", header, 0, 1, 88, 0, 0, 100, 100, 0, 0, 2500, 2500,
              0x464D4520, 0x10000, 88 + len(body), 4, 2, 0)
    pack_into("<4i", header, 72, 1000, 500, 250, 125)
    return bytes(header) + body


def wmf_codepages():
    def record(opcode, data=b""):
        data += bytes(len(data) % 2)
        return pack("<IH", 3 + len(data) // 2, opcode) + data

    def font(charset):
        return pack("<5h8B", -12, 0, 0, 0, 400, 0, 0, 0, charset, 0, 0, 0, 0) + b"Arial\0"

    records = [
        record(0x0103, pack("<H", 8)),
        record(0x020C, pack("<hh", 100, 100)),
        record(0x020E, pack("<hh", 100, 100)),
        record(0x02FC, pack("<HIH", 0, 0x000000DD, 0)),
        record(0x012D, pack("<H", 0)),
        record(0x041B, pack("<4h", 94, 94, 6, 6)),
        record(0x0209, pack("<I", 0x00EE0000)),
    ]
    for slot, (charset, label, encoding) in enumerate(((238, "Żółć", "cp1250"),
                                                       (204, "Тест", "cp1251"),
                                                       (0, "€–é", "cp1252")), 1):
        encoded = label.encode(encoding)
        records.extend([
            record(0x02FB, font(charset)),
            record(0x012D, pack("<H", slot)),
            record(0x0521, pack("<H", len(encoded)) + encoded + bytes(len(encoded) % 2)
                   + pack("<hh", 8 + 16 * slot, 17)),
        ])
    records.append(record(0))
    body = b"".join(records)
    return pack("<HHHIHIH", 1, 9, 0x300, (18 + len(body)) // 2, 4, max(map(len, records)) // 2, 0) + body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("shells", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    results = {}
    for label, kind, payload in (("emfplus", "emf", emfplus()), ("codepages", "wmf", wmf_codepages())):
        source = args.output / f"{label}.pptx"
        BASE.fixture(args.shells / f"{kind}.pptx", source, f"ppt/media/image1.{kind}", payload)
        frozen = args.output / f"{label}-frozen"
        candidate = args.output / f"{label}-candidate"
        before = BASE.render(args.frozen, source, frozen)
        after = BASE.render(args.candidate, source, candidate)
        assert before == after, (label, before, after)
        with Image.open(next(candidate.glob("*.png"))) as image:
            counts = image.convert("RGB").getcolors(image.width * image.height)
            green = sum(count for count, (r, g, b) in counts if g > 100 and r < 40 and 60 < b < 120)
        if label == "emfplus":
            assert green > 20, "split image did not draw"
            after["greenImagePixels"] = green
        results[label] = {"metafileSha256": BASE.digest(payload), "scene": after}
    (args.output / "report.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("EMF+ graphics and three Windows code pages match stage 0 pixel for pixel")


if __name__ == "__main__":
    main()
