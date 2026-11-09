#!/usr/bin/env python3
"""Regenerate native app icons from the checked-in SVGs on macOS."""
import argparse
from pathlib import Path
import struct
import subprocess
import tempfile
import xml.etree.ElementTree as ET


BRAND = Path(__file__).resolve().parents[1] / "assets/brand"
PRODUCTS = ("joyword", "joydeck", "joysheet")
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)
ICONSET_SIZES = (16, 32, 128, 256, 512)


def write_ico(output, images):
    offset = 6 + 16 * len(ICO_SIZES)
    directory = bytearray(struct.pack("<HHH", 0, 1, len(ICO_SIZES)))
    pixels = bytearray()
    for size in ICO_SIZES:
        image = images[size].read_bytes()
        dimension = size % 256
        directory.extend(struct.pack("<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(image), offset))
        pixels.extend(image)
        offset += len(image)
    output.write_bytes(directory + pixels)


def generate(product):
    ET.register_namespace("", "http://www.w3.org/2000/svg")
    source = ET.parse(BRAND / f"{product}.svg")
    with tempfile.TemporaryDirectory(prefix=product + "-icons-") as temporary:
        staging = Path(temporary)
        images = {}
        for size in sorted(set(ICO_SIZES) | set(ICONSET_SIZES) | {2 * s for s in ICONSET_SIZES}):
            image = staging / f"{size}.png"
            # Rasterize at the requested resolution, including Retina artwork.
            vector = staging / f"{size}.svg"
            source.getroot().attrib.update(width=str(size), height=str(size))
            source.write(vector, encoding="utf-8")
            subprocess.run(["sips", "-s", "format", "png", vector, "--out", image],
                           check=True, stdout=subprocess.DEVNULL)
            images[size] = image
        iconset = staging / f"{product}.iconset"
        iconset.mkdir()
        for size in ICONSET_SIZES:
            (iconset / f"icon_{size}x{size}.png").write_bytes(images[size].read_bytes())
            (iconset / f"icon_{size}x{size}@2x.png").write_bytes(images[2 * size].read_bytes())
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", staging / f"{product}.icns"], check=True)
        write_ico(staging / f"{product}.ico", images)
        for extension in ("icns", "ico"):
            name = f"{product}.{extension}"
            (BRAND / name).write_bytes((staging / name).read_bytes())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", nargs="?", choices=PRODUCTS, help="default: all products")
    args = parser.parse_args()
    for product in (args.product,) if args.product else PRODUCTS:
        generate(product)
