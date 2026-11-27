"""Replay valid EMF+ records through PPTX and check pixels against independent geometry and font data."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
from struct import pack, pack_into, unpack_from
import subprocess
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from PIL import Image, ImageChops

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-xml-vector/verify-metafiles.py"
SPEC = importlib.util.spec_from_file_location("shell", PREVIOUS)
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)
MEMBER = "ppt/media/image1.emf"
VERSION = 0xDBC01002


def plus(opcode, flags=0, data=b""):
    return pack("<HHII", opcode, flags, 12 + len(data) + (-len(data) % 4), len(data)) + data + bytes(-len(data) % 4)


def metafile(*records):
    def record(opcode, data):
        data += bytes(-len(data) % 4)
        return pack("<II", opcode, 8 + len(data)) + data

    contents = plus(0x4001, 0, pack("<4I", VERSION, 0, 96, 96)) + b"".join(records)
    body = record(70, pack("<II", len(contents) + 4, 0x2B464D45) + contents)
    body += record(14, pack("<3I", 0, 0, 20))
    header = bytearray(88)
    pack_into("<2I8i4I2H", header, 0, 1, 88, 0, 0, 100, 100, 0, 0, 2500, 2500,
              0x464D4520, 0x10000, 88 + len(body), 3, 2, 0)
    pack_into("<4i", header, 72, 1000, 500, 250, 125)
    return bytes(header) + body


def fill(color, rect=(0, 0, 100, 100)):
    return plus(0x400A, 0x8000, pack("<II4f", color, 1, *rect))


def path(*rings):
    xy = [point for ring in rings for point in ring]
    tags = b"".join(bytes([0] + [1] * (len(ring) - 2) + [0x81]) for ring in rings)
    return pack("<3I", VERSION, len(xy), 0) + b"".join(pack("<2f", *point) for point in xy) + tags


def rect(x, y, w, h):
    return pack("<I4f", 0x10000000, x, y, w, h)


def region(mode, left, right):
    return pack("<3I", VERSION, 2, mode) + left + right


RED = 0xFFDD0000
WHITE = 0xFFFFFFFF
BLUE = 0xFF0000EE


def regions():
    records = [fill(WHITE)]
    concave = path([(4, 4), (35, 4), (35, 13), (14, 13), (14, 36), (4, 36)])
    records += [plus(0x4008, 0x0301, concave), plus(0x4033, 1), fill(RED), plus(0x4031)]
    pairs = [
        (1, rect(40, 4, 17, 15), rect(47, 10, 12, 20)),
        (2, rect(62, 4, 22, 12), rect(75, 10, 19, 17)),
        (3, rect(4, 45, 23, 13), rect(15, 51, 20, 14)),
        (4, rect(40, 44, 30, 28), rect(48, 51, 12, 13)),
        (5, rect(79, 51, 9, 10), rect(73, 44, 24, 28)),
    ]
    for mode, a, b in pairs:
        records += [plus(0x4008, 0x0402, region(mode, a, b)),
                    plus(0x4034, 2), fill(RED), plus(0x4031)]
    hole = path([(4, 78), (35, 78), (35, 96), (4, 96)], [(12, 83), (25, 83), (25, 92), (12, 92)])
    records += [plus(0x4008, 0x0301, hole), plus(0x4033, 1),
                plus(0x4025, 0, pack("<I", 6)), plus(0x4035, 0, pack("<2f", 4, 0)),
                fill(RED), plus(0x4026, 0, pack("<I", 6)), plus(0x4031),
                plus(0x4032, 0x0400, pack("<4f", 61, 83, 18, 10)),
                fill(RED, (49, 78, 47, 19))]
    return metafile(*records)


def brush_solid(color):
    return pack("<3I", VERSION, 0, color)


def line(slot, *points):
    return plus(0x400D, slot, pack("<I", len(points)) + b"".join(pack("<2f", *point) for point in points))


def pens():
    def pen(slot, size, flags, optional, brush):
        return plus(0x4008, 0x0200 | slot, pack("<4If", VERSION, 0, flags, 2, size) + optional + brush)

    # Two axes, vertical first. The upper line uses a gradient pen, not a fill behind it.
    gradient = pack("<4I4f4I", VERSION, 4, 0x18, 4, 8, 4, 84, 24,
                    0xFFFF0000, 0xFF0000FF, 0, 0)
    gradient += pack("<I4fI4f", 2, 0, 1, 0, 0, 2, 0, 1, 0, 1)
    dash = pack("<IIIIfI2f", 1, 2, 1, 0, 0, 2, 3, 2)
    bands = pack("<I4f", 4, 0, .25, .75, 1)
    # Radial brush uses a polygon boundary and a different color at each vertex.
    radial = pack("<5I2fI4II8f", VERSION, 3, 0, 4, 0xFFFFFFFF, 76, 77, 4,
                  0xFF00BB22, 0xFF00BB22, 0xFF884400, 0xFF884400, 4,
                  55, 62, 96, 62, 96, 93, 55, 93)
    texture = pack("<4I", VERSION, 2, 0, 1) + pack("<7I2I", VERSION, 1, 2, 1, 8,
                                                    0x0026200A, 0, RED, BLUE)
    # Scale the two source pixels to make the mirrored tile order visible.
    texture = texture[:8] + pack("<II6f", 2, 1, 4, 0, 0, 4, 7, 91) + texture[16:]
    return metafile(
        fill(WHITE),
        pen(1, 3, 0x1CE, dash, gradient), line(1, (8, 16), (92, 16)),
        pen(2, 8, 0x400, bands, brush_solid(RED)), line(2, (10, 43), (91, 43)),
        pen(3, 6, 0xE, pack("<3I", 3, 3, 2), brush_solid(0xFF007700)),
        line(3, (12, 80), (25, 57), (43, 75)),
        plus(0x4008, 0x0105, radial), plus(0x400A, 0, pack("<II4f", 5, 1, 55, 62, 41, 31)),
        plus(0x4008, 0x0106, texture), plus(0x400A, 0, pack("<II4f", 6, 1, 7, 92, 36, 6)),
    )


def physical_font(path, characters):
    """Read cmap format 4 and hmtx directly from the test face's SFNT tables."""
    data = path.read_bytes()
    cmap_offset, head, hhea, hmtx = (
        next(unpack_from(">I", data, at + 8)[0] for at in range(12, 12 + 16 * unpack_from(">H", data, 4)[0], 16)
             if data[at:at + 4] == tag)
        for tag in (b"cmap", b"head", b"hhea", b"hmtx")
    )
    cmap_offsets = [cmap_offset + unpack_from(">I", data, at + 4)[0]
                    for at in range(cmap_offset + 4, cmap_offset + 4 + 8 * unpack_from(">H", data, cmap_offset + 2)[0], 8)
                    if unpack_from(">HH", data, at) == (3, 1)]
    table, = [at for at in cmap_offsets if unpack_from(">H", data, at)[0] == 4]
    segments = unpack_from(">H", data, table + 6)[0] // 2
    end = table + 14
    start = end + segments * 2 + 2
    delta = start + segments * 2
    offsets = delta + segments * 2
    ids = []
    for char in map(ord, characters):
        index = next(i * 2 for i in range(segments)
                     if unpack_from(">H", data, start + 2 * i)[0] <= char <= unpack_from(">H", data, end + 2 * i)[0])
        rel = unpack_from(">H", data, offsets + index)[0]
        adjustment = unpack_from(">h", data, delta + index)[0]
        gid = unpack_from(">H", data, offsets + index + rel + 2 * (char - unpack_from(">H", data, start + index)[0]))[0] if rel else char
        ids.append((gid + adjustment) & 0xFFFF if gid else 0)
    upem = unpack_from(">H", data, head + 18)[0]
    count = unpack_from(">H", data, hhea + 34)[0]
    return ids, [unpack_from(">H", data, hmtx + 4 * min(gid, count - 1))[0] / upem for gid in ids]


def glyphs(ids, advances, reference=False):
    def font(slot, family):
        return plus(0x4008, 0x0600 | slot, pack("<If4I", VERSION, 16, 0, 0, 0, len(family))
                    + family.encode("utf-16-le"))

    def draw(slot, options, codes, points, matrix=None):
        return plus(0x4036, 0x8000 | slot, pack("<4I", BLUE, options, matrix is not None, len(codes))
                    + pack(f"<{len(codes)}H", *codes)
                    + b"".join(pack("<2f", *point) for point in points)
                    + (pack("<6f", *matrix) if matrix is not None else b""))

    chars = [ord(c) for c in "ΩB"]
    records = [fill(WHITE), font(0, "Calibri"), font(1, "JoyDeck Missing Physical Face")]
    records.append(draw(0, 1 if reference else 0, chars if reference else ids, [(15, 30), (60, 49)],
                        (1, 0, 0, 1, 3, 2)))
    records.append(draw(0, 0 if reference else 4, ids,
                        [(20, 70), (20 + 16 * advances[0], 70)] if reference else [(20, 70)]))
    records.append(draw(1, 0, ids, [(15, 90), (62, 90)]))
    return metafile(*records)


def call(cli, *args):
    result = subprocess.run([str(cli.resolve()), *map(str, args)], check=True, capture_output=True,
                            encoding="utf-8", timeout=120)
    return result.stdout


def raster(cli, source, work):
    call(cli, "render", source.resolve(), "--page", "2", "--output", work.resolve())
    screenshot, = work.glob("*.png")
    with Image.open(screenshot) as pixels:
        return pixels.convert("RGB")


def picture_frame(source, image):
    with ZipFile(source) as archive:
        slide = ET.fromstring(archive.read("ppt/slides/slide2.xml"))
        matrix = slide.find(".//{*}pic/{*}spPr/{*}xfrm")
        off, ext = matrix.find("{*}off"), matrix.find("{*}ext")
        size = ET.fromstring(archive.read("ppt/presentation.xml")).find("{*}sldSz")
    xscale, yscale = image.width / int(size.get("cx")), image.height / int(size.get("cy"))
    return (int(off.get("x")) * xscale, int(off.get("y")) * yscale,
            int(ext.get("cx")) * xscale, int(ext.get("cy")) * yscale)


def pixels(image, frame, x, y):
    left, top, width, height = frame
    x, y = round(left + width * x / 100), round(top + height * y / 100)
    return image.getpixel((x, y))


def check_regions(image, frame):
    red = [(8, 26), (27, 8), (51, 14), (66, 8), (88, 21), (8, 49), (32, 62),
           (43, 47), (94, 64), (10, 88), (53, 87), (88, 86)]
    blank = [(27, 26), (43, 7), (66, 22), (20, 55), (51, 55), (83, 55), (24, 88), (70, 87)]
    for point, expected in [(p, True) for p in red] + [(p, False) for p in blank]:
        color = pixels(image, frame, *point)
        assert (color[0] > 160 and color[1] < 40 and color[2] < 40) == expected, (point, color)
    return len(red) + len(blank)


def check_pens(image, frame):
    sample = lambda x,y: pixels(image,frame,x,y)
    band = [sample(50,y) for y in (39.7,43,46.3)]
    assert band[0][1] < 45 and min(band[1]) > 230 and band[2][1] < 45, band
    row = [sample(x,16) for x in range(10,90)]
    opaque = [c for c in row if c[1] < 70]
    assert 20 < len(opaque) < 65 and opaque[0][0] > opaque[0][2] and opaque[-1][2] > opaque[-1][0], row
    assert sample(25,57)[1] > sample(25,57)[0] + 60
    center, north, south = [sample(76,y) for y in (77,65,90)]
    assert min(center) > 220 and north[1] > south[1] + 25, (center,north,south)
    sequence = [sample(x,94) for x in (9,13,17,21,25)]
    assert [r > b for r,g,b in sequence] == [True,False,False,True,True], sequence
    return {"compound": band, "dashInteriorPoints": len(opaque), "dashColors": [opaque[0],opaque[-1]],
            "pathGradient": [center,north,south], "tileFlipX": sequence}


def blue_bounds(image, frame, rect):
    l,t,w,h = frame
    x0,y0,x1,y1 = rect
    bounds = (round(l+w*x0/100),round(t+h*y0/100),round(l+w*x1/100),round(t+h*y1/100))
    area = image.crop(bounds)
    occupied = [(i % area.width + bounds[0], i // area.width + bounds[1])
                for i,(r,g,b) in enumerate(area.get_flattened_data()) if b > 160 and r < 45 and g < 45]
    if not occupied:
        return None
    x,y = zip(*occupied)
    return [min(x),min(y),max(x),max(y)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("shell", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--font", type=Path, default=Path(os.environ.get("WINDIR","C:/Windows")) / "Fonts/calibri.ttf")
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    ids, advances = physical_font(args.font, "ΩB")
    assert ids[0] > 97 and ids[1] > 0
    evidence = {"fontSha256": hashlib.sha256(args.font.read_bytes()).hexdigest(), "glyphIds": ids,
                "horizontalAdvanceEm": advances, "samples": {}}
    inputs = {"regions": regions(), "pens": pens(), "glyphs": glyphs(ids,advances),
              "glyph-reference": glyphs(ids,advances,reference=True)}
    files = {}
    for name, payload in inputs.items():
        path = args.output / f"{name}.pptx"
        BASE.fixture(args.shell, path, MEMBER, payload)
        files[name] = path
    for name, source in files.items():
        candidate = raster(args.candidate, source, args.output / f"{name}-candidate")
        frame = picture_frame(source,candidate)
        evidence["samples"][name] = {"emfSha256": BASE.digest(inputs[name]), "candidateRgbSha256": BASE.digest(candidate.tobytes())}
        if name == "glyph-reference":
            continue
        before = raster(args.frozen, source, args.output / f"{name}-frozen")
        evidence["samples"][name]["frozenRgbSha256"] = BASE.digest(before.tobytes())
        assert candidate.tobytes() != before.tobytes(), name
        if name == "regions":
            evidence["samples"][name]["checkedPoints"] = check_regions(candidate,frame)
        elif name == "pens":
            evidence["samples"][name]["measurements"] = check_pens(candidate,frame)
        else:
            reference = raster(args.candidate, files["glyph-reference"], args.output / "glyph-companion")
            locations = [(5,12,42,35),(48,27,86,53),(12,49,57,75),(0,79,100,97)]
            boxes = [blue_bounds(candidate,frame,bounds) for bounds in locations]
            expected = [blue_bounds(reference,frame,bounds) for bounds in locations]
            assert all(boxes[:3]) and boxes[3] is None and boxes == expected, (boxes,expected)
            difference = ImageChops.difference(candidate,reference).getbbox()
            assert difference is None, difference
            assert not any(blue_bounds(before,frame,bounds) for bounds in locations), name
            evidence["samples"][name]["glyphPixelBounds"] = boxes
        preserved = args.output / f"{name}-preserved.pptx"
        call(args.candidate, "exchange", source.resolve(), "--mode", "preserve", "--output", preserved.resolve())
        with ZipFile(source) as original, ZipFile(preserved) as reopened:
            assert sorted(original.namelist()) == sorted(reopened.namelist())
            for member in original.namelist():
                assert original.read(member) == reopened.read(member), member
        after = raster(args.candidate,preserved,args.output / f"{name}-reopened")
        assert after.tobytes() == candidate.tobytes(), name
        evidence["samples"][name]["preservedAllParts"] = True
    (args.output / "verification.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("Non-convex and boolean clip probes, compound and gradient pens, glyph IDs and preserved reopen passed")


if __name__ == "__main__":
    main()
