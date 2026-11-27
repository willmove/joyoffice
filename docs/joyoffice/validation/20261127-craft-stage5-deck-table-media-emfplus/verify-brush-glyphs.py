"""Compare EMF+ brush playback and verify explicit glyph baselines in raster output."""

import argparse
import importlib.util
import json
from pathlib import Path
from struct import pack, pack_into

from PIL import Image

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-xml-vector/verify-metafiles.py"
SPEC = importlib.util.spec_from_file_location("metafile_fixtures", PREVIOUS)
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)


def fixture(departure):
    def plus(opcode, flags, data=b""):
        pad = bytes(-len(data) % 4)
        return pack("<HHII", opcode, flags, 12 + len(data) + len(pad), len(data)) + data + pad

    def record(opcode, data):
        data += bytes(-len(data) % 4)
        return pack("<II", opcode, 8 + len(data)) + data

    gradient = pack("<4I4f4I", 0xDBC01002, 4, 0, 4, 18, 35, 65, 38,
                    0xFFEEAA15, 0xFF00AA40, 0, 0)
    region = pack("<4I4fI4f", 0xDBC01002, 2, 1, 0x10000000, 20, 36, 63, 30,
                  0x10000000, 23, 40, 55, 32)
    pen = pack("<4If3I", 0xDBC01002, 0, 0, 0, 2, 0xDBC01002, 0, 0xFF202020)
    family = "Arial".encode("utf-16-le")
    font = pack("<If4I", 0xDBC01002, 12, 0, 0, 0, len(family) // 2) + family
    positioned = pack("<4I2H4f", 0xFF0000EE, 1, 0, 2, ord("W"), ord("m"),
                      18, 30, 33, 30 + departure)
    parts = [
        plus(0x4001, 0, pack("<4I", 0xDBC01002, 0, 96, 96)),
        plus(0x400A, 0x8000, pack("<II4f", 0xFFDD0000, 1, 4, 4, 92, 92)),
        plus(0x4008, 0x0101, gradient),
        plus(0x4008, 0x0402, region),
        plus(0x4013, 2, pack("<I", 1)),
        plus(0x4008, 0x0204, pen),
        plus(0x400B, 4, pack("<I4f", 1, 22, 38, 58, 30)),
        plus(0x4008, 0x0603, font),
        plus(0x4036, 0x8003, positioned),
    ]
    comments = b"".join(parts)
    body = record(70, pack("<II", len(comments) + 4, 0x2B464D45) + comments)
    body += record(14, pack("<3I", 0, 0, 20))
    header = bytearray(88)
    pack_into("<2I8i4I2H", header, 0, 1, 88, 0, 0, 100, 100, 0, 0, 2500, 2500,
              0x464D4520, 0x10000, 88 + len(body), 3, 2, 0)
    pack_into("<4i", header, 72, 1000, 500, 250, 125)
    return bytes(header) + body


def inspect(output):
    image, = output.glob("*.png")
    with Image.open(image) as picture:
        rgb = picture.convert("RGB")
        blue_y = []
        gradient = set()
        gradient_pixels = 0
        for index, (r, g, b) in enumerate(rgb.get_flattened_data()):
            if b > 160 and r < 30 and g < 30:
                blue_y.append(index // rgb.width)
            if g > 130 and r < 190 and b < 80:
                gradient.add((r, g, b))
                gradient_pixels += 1
    assert blue_y and gradient_pixels >= 100
    return {"blueHeight": max(blue_y) - min(blue_y),
            "gradientColors": len(gradient), "gradientPixels": gradient_pixels}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("shells", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    evidence = {}
    for label, offset in (("same-baseline", 0), ("explicit-baselines", 37)):
        payload = fixture(offset)
        pptx = args.output / f"{label}.pptx"
        BASE.fixture(args.shells / "emf.pptx", pptx, "ppt/media/image1.emf", payload)
        samples = {}
        for key, cli in (("frozen", args.frozen), ("candidate", args.candidate)):
            dest = args.output / f"{label}-{key}"
            samples[key] = BASE.render(cli, pptx, dest) | inspect(dest)
        if offset:
            assert samples["candidate"]["blueHeight"] > samples["frozen"]["blueHeight"] + 15, samples
        else:
            assert samples["candidate"] == samples["frozen"], samples
        evidence[label] = {"emfSha256": BASE.digest(payload), "render": samples}
    (args.output / "report.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("EMF+ brush and region raster matched. Separate glyph baselines rendered at their supplied heights.")


if __name__ == "__main__":
    main()
