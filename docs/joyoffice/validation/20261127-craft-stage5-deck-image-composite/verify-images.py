"""Exercise image effects and GDI transfers through the JoyDeck PPTX renderer."""

import argparse
import importlib.util
import json
from pathlib import Path, PurePosixPath
from struct import pack, pack_into
from uuid import UUID
from xml.etree import ElementTree
from zipfile import ZipFile

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-metafile-scene/verify-metafile-scene.py"
SPEC = importlib.util.spec_from_file_location("scene", PREVIOUS)
SCENE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCENE)
DECK = SCENE.PLAYBACK
DIGEST = DECK.BASE.digest


def bitmap(*, width=8, height=8, color_at):
    pixels = b"".join(pack("<I", color_at(x, y)) for y in range(height) for x in range(width))
    return SCENE.plus(0x4008, 0x0501, pack("<7I", SCENE.VERSION, 1, width, height,
                      width * 4, 0x0026200A, 0) + pixels)


def attribute(slot, wrap, color=0, clamp=0):
    return SCENE.plus(0x4008, 0x0800 | slot, pack("<6I", SCENE.VERSION, 0, wrap, color, clamp, 0))


def effect(guid, data):
    return SCENE.plus(0x4038, 0, UUID(guid).bytes_le + pack("<I", len(data)) + data)


def draw(destination, source, *, attributes=0xFFFFFFFF, with_effect=False):
    x, y, w, h = destination
    flags = 0x2001 if with_effect else 1
    points = (x, y, x + w, y, x, y + h)
    return SCENE.plus(0x401B, flags, pack("<2I4fI6f", attributes, 2, *source, 3, *points))


def attributes_and_lut(windows):
    background = (20, 154, 74)
    red = (255, 0, 0)
    blue = (0, 0, 255)
    source = (0, 0, 8, 8)
    exterior = (-4, 0, 16, 8)
    effect_rgb = (240, 60, 100)
    lut = b"".join(bytes([value]) * 256 for value in (100, 60, 240, 127))
    records = [
        SCENE.fill(0xFF149A4A), bitmap(color_at=lambda x, y: 0xFFFF0000 if x < 4 else 0xFF0000FF),
        attribute(2, 4, 0xFF00DCF0), attribute(3, 1), attribute(4, 0), attribute(5, 4, 0xFF00DCF0, 1),
    ]
    for i, slot in enumerate((2, 3, 4, 5)):
        # Exercise both DrawImage encodings.
        if slot == 2:
            records.append(SCENE.plus(0x401A, 1, pack("<2I8f", slot, 2, *exterior, 10, 5, 80, 15)))
        else:
            records.append(draw((10, 5 + i * 20, 80, 15), exterior, attributes=slot))
    records += [
        effect("A7CE72A9-0F7F-40D7-B3CC-D0C02D5C3212", lut),
        draw((4, 86, 22, 10), source, with_effect=True),
        SCENE.plus(0x4025, 0, pack("<I", 9)), SCENE.plus(0x4023, 1),
        draw((29, 86, 23, 10), source, with_effect=True),
        SCENE.plus(0x4026, 0, pack("<I", 9)),
        draw((55, 86, 23, 10), source, with_effect=True),
        draw((81, 86, 15, 10), source),
    ]

    def over(bottom):
        return tuple(round((s * 127 + d * 128) / 255) for s, d in zip(effect_rgb, bottom, strict=True))

    probes = []
    for y, colors in zip((12, 32, 52, 72), windows["attributes"], strict=True):
        probes.extend(zip(((20, y), (39, y), (60, y), (81, y)), colors, strict=True))
    probes += [((15, 91), over(background)), ((41, 91), over((255, 255, 255))),
               ((68, 91), over(background)), ((84, 91), red), ((93, 91), blue),
               ((2, 91), background)]
    return SCENE.emfplus(*records), probes


def filters():
    source = (0, 0, 8, 8)
    records = [SCENE.fill(SCENE.WHITE), bitmap(color_at=lambda x, y: 0xFF40648C)]
    probes = []

    def swatch(guid, parameters, dest, samples):
        records.append(effect(guid, parameters))
        records.append(draw(dest, source, with_effect=True))
        probes.extend(samples)

    swatch("D3A1DBE1-8EC4-4C17-9F4C-EA97AD1C343D", pack("<2i", 32, 0),
           (4, 5, 25, 22), [((15, 15), (96, 132, 172))])
    swatch("DD6A0022-58E4-4A67-9D9B-D48EB881A53D", pack("<3i", 6, 1, 128),
           (36, 5, 25, 22), [((47, 15), (128, 100, 140))])
    swatch("8B2DD6C3-EB07-4D87-A5F0-7108E26A9C5F", pack("<3i", 120, 0, 0),
           (68, 5, 25, 22), [((79, 15), (140, 64, 100))])
    swatch("99C354EC-2A31-4F3A-8C34-17A803B33A25", pack("<3i", 100, 0, 0),
           (4, 31, 25, 22), [((15, 40), (64, 100, 140))])
    # Bounds verify the direction of these adjustments without assuming a native tone curve.
    swatch("537E597D-251E-48DA-9664-29CA496B70F8", pack("<3i", 35, 0, 0),
           (36, 31, 25, 22), [((47, 40), ((80, 220), 100, 140))])
    swatch("1077AF00-2848-4441-9489-44AD4C2D7A2C", pack("<2i", -140, 30),
           (68, 31, 25, 22), [((79, 40), ((75, 245), (106, 245), (141, 255)))])
    records.append(bitmap(color_at=lambda x, y: 0xFFC8283C))
    swatch("74D29D05-69A4-4266-9549-3CC52836B632", pack("<5i", 1, 0, 0, 4, 8),
           (4, 57, 25, 33), [((9, 72), ((30, 80), 40, 60)), ((25, 72), (200, 40, 60))])
    records.append(bitmap(color_at=lambda x, y: 0xFFFF0000 if 2 <= x <= 5 and 2 <= y <= 5 else 0))
    swatch("633C80A4-1843-482B-9EF2-BE2834C5FDD4", pack("<fI", 1, 1),
           (38, 63, 24, 24), [((49, 75), (255, 0, 0)), ((42.5, 75), (255, (200, 245), (200, 245))),
                            ((49, 67.5), (255, (200, 245), (200, 245))), ((38, 75), (255, 255, 255))])
    records.append(bitmap(color_at=lambda x, y: 0xFFC0C0C0 if x >= 4 else 0xFF505050))
    swatch("63CBF3EE-C526-402C-8F71-62C540BF5142", pack("<2f", 2, 100),
           (68, 59, 28, 32), [((80, 75), ((0, 65),) * 3), ((84, 75), ((213, 255),) * 3)])
    return SCENE.emfplus(*records), probes


def dib(width, height, depth, color_at):
    rows = []
    for y in range(abs(height)):
        row = bytearray()
        for x in range(width):
            color = color_at(x, y)
            if depth == 1:
                if x % 8 == 0:
                    row.append(0)
                row[-1] |= color << (7 - x % 8)
            else:
                r, g, b, *a = color
                row.extend([b, g, r] + a)
        rows.append(bytes(row) + bytes(-len(row) % 4))
    pixels = b"".join(rows if height < 0 else reversed(rows))
    palette = pack("<2I", 0, 0x00FFFFFF) if depth == 1 else b""
    return pack("<IiiHH6I", 40, width, height, 1, depth, 0, len(pixels), 0, 0, 0, 0) + palette, pixels


def transfer(kind, dest, source, *, rop=0, planes=None, mask=None, origin=(0, 0), dx=0):
    header_size = {76: 100, 78: 128, 79: 140, 114: 108, 116: 108}[kind]
    header = bytearray(header_size)
    pack_into("<4i", header, 8, 0, 0, 100, 100)
    if kind == 79:
        pack_into("<10i6f", header, 24, *dest, *source, 1, 0, 0, 1, dx, 0)
        entries = [(96, planes), (124, mask)]
        pack_into("<2i", header, 112, *origin)
    else:
        pack_into("<4iI2i6f", header, 24, *dest, rop, *source[:2], 1, 0, 0, 1, dx, 0)
        entries = [(84, planes)]
        if kind == 78:
            entries.append((112, mask))
            pack_into("<2i", header, 100, *origin)
        elif kind in (114, 116):
            pack_into("<2i", header, 100, *source[2:])
    payload = b""
    for field, data in entries:
        if data is None:
            continue
        info, bits = data
        offset = header_size + len(payload)
        pack_into("<4I", header, field, offset, len(info), offset + len(info), len(bits))
        payload += info + bits
    return kind, bytes(header[8:]) + payload


def gdi():
    base = (170, 85, 51)
    src = (60, 150, 204)
    inverted = tuple(255 - x for x in base)
    colorref = lambda rgb: int.from_bytes(bytes(rgb), "little")
    mask_bit = lambda x, y: int((x % 9 < 4) != (y % 2 == 0))
    mask = dib(9, 2, 1, mask_bit)
    stripes = dib(46, 28, 24, lambda x, y: src)
    plg = dib(13, -8, 24, lambda x, y: (253, 45, 9) if x < 6 else (23, 71, 242))
    key = (200, 10, 20)
    opaque = (30, 190, 80)
    keyed = dib(8, -8, 24, lambda x, y: key if x < 4 else opaque)
    blend = dib(8, 8, 32, lambda x, y: (100, 20, 40, 128))
    records = [
        (39, pack("<4I", 1, 0, colorref(base), 0)), (37, pack("<I", 1)),
        (37, pack("<I", 0x80000008)), (43, pack("<4i", 0, 0, 100, 100)),
        transfer(78, (8, 8, 32, 24), (0, 0, 32, 24), rop=0xAACC0020,
                 planes=stripes, mask=mask, origin=(2, 1)),
        (33, b""), (30, pack("<4i", 54, 8, 88, 32)),
        transfer(78, (51, 8, 39, 24), (0, 0, 39, 24), rop=0x668800C6,
                 planes=stripes, mask=mask, origin=(1, 0)),
        (34, pack("<i", -1)),
        transfer(79, (9, 40, 45, 45, 16, 63), (1, 0, 8, 8), planes=plg,
                 mask=dib(8, -8, 1, lambda x, y: int(y < 4)), dx=1),
        transfer(116, (57, 41, 33, 22), (0, 0, 8, 8), rop=colorref(key), planes=keyed),
        transfer(114, (9, 72, 34, 20), (0, 0, 8, 8), rop=0x01800000, planes=blend),
        transfer(76, (49, 72, 42, 20), (0, 0, 42, 20), rop=0x00550009),
    ]
    probes = []
    for x, y in ((10.5, 11.5), (13.5, 11.5), (21.5, 14.5), (22.5, 15.5),
                 (28.5, 15.5), (29.5, 15.5), (34.5, 17.5)):
        mask_value = mask_bit(int(x - 8 + 2), int(y - 8 + 1))
        probes.append(((x, y), src if mask_value else base))
    for x, y in ((51.5, 10.5), (55.5, 12.5), (56.5, 12.5), (60.5, 11.5),
                 (67.5, 18.5), (72.5, 18.5), (85.5, 14.5), (88.5, 14.5)):
        op = int.__and__ if mask_bit(int(x - 51 + 1), int(y - 8)) else int.__xor__
        probes.append(((x, y), tuple(map(op, base, src)) if 54 < x < 88 else base))
    for u, v, rgb in ((.20, .20, (253, 45, 9)), (.75, .22, (23, 71, 242)),
                      (.20, .72, base), (.75, .73, base), (.60, -.20, base)):
        probes.append(((9 + u * 36 + v * 7, 40 + u * 5 + v * 23), rgb))
    probes += [((63, 50), base), ((84, 50), opaque), ((18, 82), (177, 74, 58)),
               ((65, 82), inverted), ((96, 82), base)]
    return SCENE.wrap_emf(records), probes


def office_media(file):
    folder = PurePosixPath("ppt/slides")
    with ZipFile(file) as archive:
        slide = ElementTree.fromstring(archive.read(str(folder / "slide2.xml")))
        tag = "{http://schemas.openxmlformats.org/drawingml/2006/main}blip"
        embed = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}embed"
        references = {entry.attrib[embed] for entry in slide.iter(tag) if embed in entry.attrib}
        rels = ElementTree.fromstring(archive.read(str(folder / "_rels/slide2.xml.rels")))
        for rel in rels:
            target = rel.attrib["Target"]
            if rel.attrib["Id"] in references and target.endswith(".emf"):
                assert target.startswith("../"), target
                return archive.read(str(folder.parent / target[3:]))
    raise AssertionError(f"No related EMF on slide 2: {file}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("frozen", "candidate", "shell", "native", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--office", type=Path)
    args = parser.parse_args()
    report = args.output / "verification.json"
    windows = json.loads((args.native / "windows.json").read_text(encoding="utf-8-sig"))
    if args.office:
        evidence = json.loads(report.read_text(encoding="utf-8"))
        office = json.loads((args.office / "verification.json").read_text(encoding="utf-8-sig"))
        checked = set()
        for case in office["cases"]:
            name = Path(case["input"]).stem.removesuffix("-preserved")
            file = args.office / case["output"]
            payload = office_media(file)
            assert DIGEST(payload) == evidence[name]["emfSha256"], name
            actual = DECK.raster(args.candidate, file, args.output / f"{name}-office")
            assert DIGEST(actual.tobytes()) == evidence[name]["candidateRgbSha256"], name
            evidence[name]["powerPointEmfAndReopenedRasterExact"] = True
            checked.add(name)
        assert checked == {name for name, entry in evidence.items() if "emfSha256" in entry}
        evidence["powerPoint"] = office
        evidence["windowsOracle"] = windows
    else:
        args.output.mkdir(parents=True, exist_ok=True)
        fixtures = {
            "matrix": ((args.native / "matrix-native.emf").read_bytes(), [((45, 45), windows["matrix"])]),
            "attributes-lut": attributes_and_lut(windows),
            "gdi": gdi(),
            "filters": filters(),
        }
        evidence = {}
        for name, (payload, probes) in fixtures.items():
            original = args.output / f"{name}.pptx"
            SCENE.enlarged_shell(args.shell, original, payload)
            candidate = DECK.raster(args.candidate, original, args.output / f"{name}-candidate")
            frozen = DECK.raster(args.frozen, original, args.output / f"{name}-frozen")
            frame = DECK.picture_frame(original, candidate)
            values = []
            for point, expected in probes:
                found = SCENE.pixel(candidate, frame, point)
                values.append({"point": point, "expected": expected, "candidate": found,
                               "frozen": SCENE.pixel(frozen, frame, point)})
                def matches(channel, reference):
                    low, high = (reference - 3, reference + 3) if isinstance(reference, int) else reference
                    return low <= channel <= high
                assert all(matches(a, b) for a, b in zip(found, expected, strict=True)), (name, values[-1])
            assert candidate.tobytes() != frozen.tobytes(), name
            preserved = args.output / f"{name}-preserved.pptx"
            DECK.call(args.candidate, "exchange", original.resolve(), "--mode", "preserve",
                      "--output", preserved.resolve())
            with ZipFile(original) as before, ZipFile(preserved) as after:
                assert sorted(before.namelist()) == sorted(after.namelist())
                assert all(before.read(part) == after.read(part) for part in before.namelist())
            reopened = DECK.raster(args.candidate, preserved, args.output / f"{name}-reopened")
            assert reopened.tobytes() == candidate.tobytes()
            evidence[name] = {"emfSha256": DIGEST(payload), "candidateRgbSha256": DIGEST(candidate.tobytes()),
                              "frozenRgbSha256": DIGEST(frozen.tobytes()), "preservedPartsExact": True,
                              "probes": values}
    report.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("Passed:", ", ".join(evidence))


if __name__ == "__main__":
    main()
