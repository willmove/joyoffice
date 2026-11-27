"""Exercise production metafile rendering and preserving PPTX exchange against independent probes."""

import argparse
import importlib.util
import json
from pathlib import Path
from struct import pack, pack_into
import re
from zipfile import ZIP_DEFLATED, ZipFile

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-complex-emfplus/verify-playback.py"
SPEC = importlib.util.spec_from_file_location("playback", PREVIOUS)
PLAYBACK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PLAYBACK)
plus, fill, path, line = (PLAYBACK.plus, PLAYBACK.fill, PLAYBACK.path, PLAYBACK.line)
VERSION, WHITE, BLUE = PLAYBACK.VERSION, PLAYBACK.WHITE, PLAYBACK.BLUE
RED = 0xFFFF0000
COLORS = {"red": (255, 0, 0), "green": (0, 255, 0), "blue": (0, 0, 255), "yellow": (255, 255, 0)}


def wrap_emf(records):
    def record(opcode, data):
        data += bytes(-len(data) % 4)
        return pack("<II", opcode, 8 + len(data)) + data

    body = b"".join(record(code, data) for code, data in records)
    body += record(14, pack("<3I", 0, 0, 20))
    header = bytearray(88)
    # Device frame 100x100. A valid physical frame also lets Office embed this content.
    pack_into("<2I8i4I2H", header, 0, 1, 88, 0, 0, 100, 100, 0, 0, 2500, 2500,
              0x464D4520, 0x10000, 88 + len(body), len(records) + 2, 2, 0)
    pack_into("<4i", header, 72, 1000, 500, 250, 125)
    return bytes(header) + body


def emfplus(*records):
    stream = plus(0x4001, 0, pack("<4I", VERSION, 0, 96, 96)) + b"".join(records)
    return wrap_emf([(70, pack("<II", len(stream) + 4, 0x2B464D45) + stream)])


def enlarged_shell(source, target, payload):
    slide = "ppt/slides/slide2.xml"
    with ZipFile(source) as archive, ZipFile(target, "w", ZIP_DEFLATED) as copy:
        for member in archive.namelist():
            data = archive.read(member)
            if member == slide:
                original = b'<a:off x="4114800" y="2743200"/><a:ext cx="2194560" cy="1463040"/>'
                larger = b'<a:off x="635000" y="508000"/><a:ext cx="10922000" cy="5842000"/>'
                assert data.count(original) == 1
                data = data.replace(original, larger, 1)
                # Retain the shell's compatibility markup and media relationship. Remove slide
                # decorations whose opaque fills would cover probes in the enlarged picture.
                data = re.sub(rb"<p:(sp|graphicFrame)>.*?</p:\1>", b"", data, flags=re.DOTALL)
            if member == PLAYBACK.MEMBER:
                data = payload
            copy.writestr(member, data)


def hatch_scene():
    records = [fill(WHITE)]
    for index, style in enumerate(range(6, 53)):
        x, y = hatch_origin(index)
        if index == 0:
            records += [plus(0x4025, 0, pack("<I", 9)), plus(0x401D, 0, pack("<2i", 3, 5))]
        brush = pack("<5I", VERSION, 1, style, 0xFF00FF00, 0xFFFF00FF)
        records.extend([plus(0x4008, 0x0101, brush), plus(0x400A, 0, pack("<II4f", 1, 1, x, y, 9, 9))])
        if index == 0:
            records.append(plus(0x4026, 0, pack("<I", 9)))
    return emfplus(*records)


def hatch_origin(index):
    return (2 + (index % 8) * 12, 3 + (index // 8) * 15)


def text_scene():
    # Glyph coordinates cross both sloped edges but stay well within the old bounding rectangle.
    ring = [(50, 12), (90, 50), (50, 88), (10, 50)]
    family = "Arial"
    font = pack("<If4I", VERSION, 17, 0, 0, 0, len(family)) + family.encode("utf-16-le")
    text = "MMMMMM"
    origins = [20, 34, 48, 62, 76, 83]
    draw = pack("<4I", BLUE, 1, 0, len(text)) + text.encode("utf-16-le")
    draw += b"".join(pack("<2f", x, 43) for x in origins)
    return emfplus(
        fill(WHITE), plus(0x4008, 0x0301, path(ring)), plus(0x4033, 1),
        plus(0x4008, 0x0602, font), plus(0x4036, 0x8002, draw),
    )


def quad(dest, source=(0, 0, 8, 8), slot=1, mode="float"):
    flags = slot
    if mode == "short":
        data = b"".join(pack("<2h", *point) for point in dest)
        flags |= 0x4000
    elif mode == "relative":
        def encode(value):
            return bytes([value & 0x7F]) if -64 <= value <= 63 else pack(">H", (value & 0x7FFF) | 0x8000)
        previous = (0, 0)
        data = b""
        for point in dest:
            data += encode(point[0] - previous[0]) + encode(point[1] - previous[1])
            previous = point
        flags |= 0x0800
    else:
        data = b"".join(pack("<2f", *point) for point in dest)
    return plus(0x401B, flags, pack("<2I4fI", 0xFFFFFFFF, 2, *source, 3) + data)


def bitmap_scene():
    texels = [
        (0xFFFF0000, 0xFF00FF00) if row < 4 else (0xFF0000FF, 0xFFFFFF00)
        for row in range(8)
    ]
    pixels = b"".join(pack("<8I", *((a,) * 4 + (b,) * 4)) for a, b in texels)
    bitmap = pack("<7I", VERSION, 1, 8, 8, 32, 0x0026200A, 0) + pixels
    return emfplus(
        fill(WHITE), plus(0x4008, 0x0501, bitmap),
        quad(((39, 8), (39, 40), (7, 8))),                             # 90 degrees
        quad(((91, 8), (56, 13), (97, 40)), mode="short"),             # reflected and sheared
        quad(((9, 63), (39, 63), (9, 94)), source=(8, 0, -8, 8)),       # reverse signed X source
        plus(0x4025, 0, pack("<I", 6)), plus(0x402D, 0, pack("<2f", -20, 0)),
        quad(((77, 62), (114, 62), (77, 94)), source=(-2, 0, 8, 8), mode="relative"),
        plus(0x4026, 0, pack("<I", 6)),                              # signed 7- and 15-bit deltas
        plus(0x4025, 0, pack("<I", 7)),
        plus(0x402A, 0, pack("<6f", 0, -1, 1, 0, 61, 60)),
        plus(0x401A, 1, pack("<2I8f", 0xFFFFFFFF, 2, 0, 0, 8, 8, 0, 0, 14, 14)),
        plus(0x4026, 0, pack("<I", 7)),                              # rotated DrawImage rectangle
    )


def gdi_scene():
    top = bytes((0, 0, 255, 0)) * 4 + bytes((0, 255, 0, 0)) * 4
    bottom = bytes((255, 0, 0, 0)) * 4 + bytes((0, 255, 255, 0)) * 4
    pixels = top * 4 + bottom * 4
    bmi = pack("<IiiHH6I", 40, 8, -8, 1, 32, 0, len(pixels), 0, 0, 0, 0)
    def stretch(x, y, source_x, source_width, width, height):
        return (pack("<4i6i4I2I2i", 0, 0, 100, 100, x, y, source_x, 0, source_width, 8,
                     80, 40, 120, len(pixels), 0, 0x00CC0020, width, height) + bmi + pixels)
    return wrap_emf([
        (43, pack("<4i", 1, 1, 99, 99)), (35, pack("<6f", 0, 1, -1, 0, 70, 22)),
        (81, stretch(0, 0, 0, 8, 40, 40)), (35, pack("<6f", 1, 0, 0, 1, 0, 0)),
        (81, stretch(15, 70, 8, -8, 50, 25)),
    ])


def nested_scene():
    inner = text_scene()
    metafile = pack("<4I", VERSION, 2, 3, len(inner)) + inner
    return emfplus(fill(WHITE), plus(0x4008, 0x0501, metafile),
                   quad(((90, 10), (90, 90), (10, 10)), source=(10, 10, 80, 80)))


def pixel(image, frame, point):
    return PLAYBACK.pixels(image, frame, *point)


def check_hatches(image, frame, native):
    observed = 0
    for index, record in enumerate(native):
        x, y = hatch_origin(index)
        side, bits = record["side"], record["bits"]
        assert record["style"] == index + 6
        dx, dy = (3, 5) if index == 0 else (0, 0)
        for row in range(8):
            for column in range(8):
                expected = bits[((y + row - dy) % 8)*side + ((x + column - dx) % 8)]
                actual = pixel(image,frame,(x + column + .5,y + row + .5))
                color = (0, 255, 0) if expected else (255, 0, 255)
                assert max(abs(a-b) for a,b in zip(actual,color)) <= 30, (record["style"],column,row,expected,actual)
                observed += 1
    return {"styles": len(native), "probes": observed}


def check_tiles(image, frame, gdi=False):
    def verify(coordinates, values):
        for coordinate, value in zip(coordinates, values, strict=True):
            actual = pixel(image, frame, coordinate)
            expect = COLORS[value]
            assert max(abs(a-b) for a,b in zip(actual,expect)) < 32, (coordinate,actual,value)

    palette = ["red", "green", "blue", "yellow"]
    if gdi:
        verify([(60, 32), (60, 52), (40, 32), (40, 52)], palette)
        verify([(28,76),(54,76),(28,89),(54,89)], ["green","red","yellow","blue"])
        return 8
    verify([(31,16),(31,32),(15,16),(15,32)], palette)
    verify([(84,18),(67,20),(87,34),(70,36)], palette)
    verify([(16,70),(33,70),(16,88),(33,88)], ["green","red","yellow","blue"])
    verify([(71,70),(89,70),(71,86),(89,86)], palette)
    verify([(64,56),(64,49),(71,56),(71,49)], palette)
    assert min(pixel(image, frame, (60, 74))) > 230                 # cropped source extends off bitmap
    return 21


def colored_points(image, frame, color):
    left, top, w, h = frame
    right, bottom = round(left + w), round(top + h)
    result = []
    for y in range(round(top),bottom):
        for x in range(round(left),right):
            rgb = image.getpixel((x,y))
            if rgb[color] > 185 and max(rgb[c] for c in range(3) if c != color) < 55:
                result.append(((x+.5-left)*100/w,(y+.5-top)*100/h))
    return result


def glyph_probe(image, frame, nested=False):
    glyphs = colored_points(image,frame,2)
    assert len(glyphs) > 100, len(glyphs)
    # Reverse the nested image's crop and clockwise quarter turn before testing its child clip.
    if nested:
        glyphs = [(y, 100-x) for x,y in glyphs]
    outside = sum(abs(x-50)/40 + abs(y-50)/38 > 1.0 for x,y in glyphs)
    return {"bluePixels": len(glyphs), "outsideDiamond": outside}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("shell", type=Path)
    parser.add_argument("oracle", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--only", choices=("caps","hatch","clip","bitmap","gdi","nested"))
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    native = json.loads((args.oracle/"hatches.json").read_text(encoding="utf-8-sig"))
    samples = {
        "caps": (args.oracle/"windows-caps.emf").read_bytes(), "hatch": hatch_scene(),
        "clip": text_scene(), "bitmap": bitmap_scene(), "gdi": gdi_scene(), "nested": nested_scene(),
    }
    evidence = {}
    for name,payload in samples.items():
        if args.only and name != args.only:
            continue
        source = args.output/f"{name}.pptx"
        enlarged_shell(args.shell,source,payload)
        candidate = PLAYBACK.raster(args.candidate,source,args.output/f"{name}-candidate")
        frozen = PLAYBACK.raster(args.frozen,source,args.output/f"{name}-frozen")
        frame = PLAYBACK.picture_frame(source,candidate)
        target = {"emfSha256": PLAYBACK.BASE.digest(payload),
                  "candidateRgbSha256": PLAYBACK.BASE.digest(candidate.tobytes()),
                  "frozenRgbSha256": PLAYBACK.BASE.digest(frozen.tobytes())}
        assert target["candidateRgbSha256"] != target["frozenRgbSha256"], name
        if name == "caps":
            a,b = colored_points(candidate,frame,0),colored_points(frozen,frame,0)
            lo,hi = min(x for x,_ in b),max(x for x,_ in b)
            start = sum(x < lo-3 for x,_ in a)
            end = sum(x > hi+3 for x,_ in a)
            assert start > 100 and end > 100, (start,end,lo,hi)
            target["measurements"] = {"startCapOutsideStem": start, "endCapOutsideStem": end}
        elif name == "hatch":
            target["measurements"] = check_hatches(candidate,frame,native)
        elif name in ("bitmap","gdi"):
            target["measurements"] = {"colorProbes": check_tiles(candidate,frame,name=="gdi")}
        else:
            measurements = glyph_probe(candidate,frame,name=="nested")
            assert measurements["outsideDiamond"] == 0, measurements
            before = glyph_probe(frozen,frame,name=="nested")
            assert before["outsideDiamond"] > 100
            measurements["frozenOutsideDiamond"] = before["outsideDiamond"]
            target["measurements"] = measurements

        preserved = args.output/f"{name}-preserved.pptx"
        PLAYBACK.call(args.candidate,"exchange",source.resolve(),"--mode","preserve","--output",preserved.resolve())
        with ZipFile(source) as original, ZipFile(preserved) as output:
            assert sorted(original.namelist()) == sorted(output.namelist())
            assert all(original.read(member)==output.read(member) for member in original.namelist())
        reopened = PLAYBACK.raster(args.candidate,preserved,args.output/f"{name}-reopened")
        assert reopened.tobytes() == candidate.tobytes(), name
        target["preservedPartsExact"] = True
        evidence[name] = target
    (args.output/"verification.json").write_text(json.dumps(evidence,indent=2)+"\n",encoding="utf-8",newline="\n")
    print("Passed:", ", ".join(evidence))


if __name__ == "__main__":
    main()
