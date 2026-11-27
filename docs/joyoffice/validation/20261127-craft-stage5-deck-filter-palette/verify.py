"""Verify native blur, WMF indexed playback, brushed glyphs and PPTX round trips."""

import argparse
import importlib.util
import json
from collections import Counter
from itertools import product
from pathlib import Path
from struct import pack, unpack_from
from zipfile import ZipFile

from PIL import Image

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-vector-copy/verify-vectors.py"
SPEC = importlib.util.spec_from_file_location("vectors", PREVIOUS)
VECTORS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VECTORS)
SCENE, DECK, DIGEST = VECTORS.SCENE, VECTORS.DECK, VECTORS.DIGEST


def bitmap16(depth, rows, pattern=False):
    width = len(rows[0])
    stride = (width * depth + 15) // 16 * 2
    pixels = bytearray()
    for row in rows:
        if depth == 4:
            nibbles = row + ([15] if width % 2 else [])
            row = [nibbles[i] * 16 + nibbles[i + 1] for i in range(0, len(nibbles), 2)]
        pixels.extend(bytes(row) + bytes([0xFE]) * (stride - len(row)))
    return pack("<4h2B", 0, width, len(rows), stride, 1, depth) + bytes(22 if pattern else 0) + pixels


def wmf_scene(palettes):
    colors = [(18, 41, 93, 0), (170, 40, 12, 1), (5, 201, 98, 0), (41, 0, 0, 2)]
    alternate = [(90, 80, 70, 0), (46, 39, 240, 0), (36, 102, 201, 0)]
    updated, animated = (65, 76, 87, 0), (9, 77, 143, 0)
    rows4 = [[0, 1, 2, 3, 15], [9, 8, 7, 6, 5], [1, 3, 2, 0, 6]]
    rows8 = [[1, 3, 15, 26, 40, 73, 255], [2, 41, 79, 123, 183, 222, 0]]
    pattern = [[1, 2, 3], [2, 3, 1]]
    records = [
        (0x0103, pack("<H", 8)), (0x020B, pack("<2h", 0, 0)), (0x020C, pack("<2h", 100, 100)),
        (0x020D, pack("<2h", 0, 0)), (0x020E, pack("<2h", 100, 100)),
    ]
    probes = []

    def palette(start, entries):
        return pack("<2H", start, len(entries)) + b"".join(bytes(color) for color in entries)

    def select(index):
        records.append((0x0234, pack("<H", index)))

    def rgb(depth, table, index):
        if index >= len(table):
            return palettes[str(depth)][index]
        r, g, b, flags = table[index]
        return palettes["8"][r + 256 * g] if flags == 2 else [r, g, b]

    def blit(x, y, rows, depth, table, stretch):
        w, h = len(rows[0]), len(rows)
        scale = 4 if stretch else 1
        rop = 0x00CC0020
        if stretch:
            fields = (0x0B23, pack("<I8h", rop, h, w, 0, 0, h * scale, w * scale, y, x))
        else:
            fields = (0x0922, pack("<I6h", rop, 0, 0, h, w, y, x))
        records.append((fields[0], fields[1] + bitmap16(depth, rows)))
        for row, indices in enumerate(rows):
            probes.extend(((x + (col + .5) * scale, y + (row + .5) * scale), rgb(depth, table, index))
                          for col, index in enumerate(indices))

    records += [(0x00F7, palette(0x300, colors)), (0x00F7, palette(0x300, alternate))]
    blit(6, 5, rows4, 4, [], True)
    blit(34, 5, rows8, 8, [], True)
    select(0)
    records.append((0x0035, b""))
    blit(6, 24, rows4, 4, colors, True)
    records.append((0x001E, b""))
    select(1)
    blit(34, 24, rows8, 8, alternate, True)
    records.append((0x0127, pack("<h", -1)))
    blit(67, 24, rows8, 8, colors, True)
    blit(66, 40, rows4, 4, colors, False)
    records += [(0x01F9, bitmap16(4, pattern, pattern=True)), (0x012D, pack("<H", 2))]
    records += [(0x0037, palette(2, [updated])), (0x0436, palette(1, [animated, alternate[2]]))]
    effective = [colors[0], animated, updated, colors[3]]
    blit(6, 48, rows4, 4, effective, True)
    # Realize changes visible indices in the already-created pattern too.
    records.append((0x0922, pack("<I7h", 0x00F00021, 0, 0, 0, 12, 30, 65, 6)))
    for x, y in ((7, 67), (8, 67), (9, 67), (7, 68), (8, 68), (9, 68)):
        probes.append(((x + .5, y + .5), rgb(4, effective, pattern[y % 2][x % 3])))
    select(0x800F)
    blit(50, 72, rows8, 8, [], True)
    records.append((0, b""))
    parts = [VECTORS.wmf_record(*record) for record in records]
    return (pack("<HHHIHIH", 1, 9, 0x300, 9 + sum(map(len, parts)) // 2,
                 3, max(map(len, parts)) // 2, 0) + b"".join(parts)), probes


def indexed_glyphs():
    version = 0xDBC01002

    def plus(op, flags, body):
        padding = bytes(-len(body) % 4)
        return pack("<HHII", op, flags, 12 + len(body) + len(padding), len(body)) + body + padding

    def draw(flags, brush, y):
        # Arial glyph 43 is H. Equal baselines and overlapping origins exercise the run mask.
        return plus(0x4036, flags | 2, pack("<4I3H6f", brush, 0, 0, 3, 43, 43, 43,
                                          8, y, 25, y, 26, y))

    family = "Arial".encode("utf-16-le")
    parts = [plus(0x4001, 0, pack("<4I", version, 0, 96, 96)),
             plus(0x4009, 0, pack("<I", 0xFF00FF00)),
             plus(0x4008, 0x0602, pack("<If4I", version, 26, 0, 0, 0, 5) + family),
             plus(0x4008, 0x0101, pack("<4I4f4I", version, 4, 0, 4, 0, 0, 99, 100,
                                      0x00FF0000, 0xA0FF0000, 0, 0)),
             draw(0, 1, 24),
             plus(0x4023, 1, b""), draw(0, 1, 53),
             draw(0x8000, 0x00000000, 82)]
    comments = b"".join(parts)
    return SCENE.wrap_emf([(70, pack("<II", len(comments) + 4, 0x2B464D45) + comments)])


def device_frame(payload):
    left, top, right, bottom = unpack_from("<4i", payload, 24)
    px, py, mmx, mmy = unpack_from("<4i", payload, 72)
    sx, sy = px / (100 * mmx), py / (100 * mmy)
    return left * sx, top * sy, (right - left) * sx, (bottom - top) * sy


def frame_points(payload, points):
    left, top, width, height = device_frame(payload)
    return [((x - left) * 100 / width, (y - top) * 100 / height)
            for x, y in points]


def blur_probes(payload, frame, native, name):
    left, top, width, height = frame
    dx, dy, dw, dh = device_frame(payload)
    offsets = range(-30, 31, 3) if name == "blur-255" else range(-6, 7)
    points = [(50 + i, 22) for i in offsets] + [(22, 50 + i) for i in offsets]
    with Image.open(native / f"{name}-bitmap.png") as png:
        ref = png.convert("RGB")
    probes = []
    for u, v in frame_points(payload, points):
        # Reverse the PPTX sampling transform at the actual raster pixel center.
        x = dx + (round(left + width * u / 100) + .5 - left) * dw / width
        y = dy + (round(top + height * v / 100) + .5 - top) * dh / height
        source = (int((x - 5) / .15), int((y - 5) / .15))
        texels = [ref.getpixel((source[0] + i, source[1] + j)) for i, j in product((-1, 0, 1), repeat=2)]
        probes.append(((u, v), ref.getpixel(source), texels))
    return probes


def color_counts(image, frame, payload, *, native=False, indexed=False):
    # Glyph edges are deliberately omitted: GDI+ and the outline rasterizer use different hinting.
    bands = ((4, 26), (32, 55), (63, 86)) if indexed else ((6, 27), (36, 57), (66, 87))
    result = []
    for top, bottom in bands:
        points = [(x + .5, y + .5) for y in range(top, bottom) for x in range(7, 50 if indexed else 82)]
        if native:
            pixels = [image.getpixel((int(x), int(y))) for x, y in points]
        elif indexed:
            pixels = [SCENE.pixel(image, frame, point) for point in points]
        else:
            pixels = [SCENE.pixel(image, frame, point) for point in frame_points(payload, points)]
        result.append({"over": sum(r > 7 and b <= 3 and abs(r + g - 255) <= 3 for r, g, b in pixels),
                       "copy": sum(r > 240 and g > 75 and abs(b - g) <= 3 and b < 245 for r, g, b in pixels),
                       "clear": pixels.count((255, 255, 255)),
                       "untouched": pixels.count((0, 255, 0)),
                       "copyShades": len({g for r, g, b in pixels if r > 240 and abs(b - g) <= 3 and b < 245})})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("frozen", "candidate", "shells", "native", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--office", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    report = args.output / "verification.json"
    if args.office:
        evidence = json.loads(report.read_text(encoding="utf-8"))
        office = json.loads((args.office / "verification.json").read_text(encoding="utf-8-sig"))
        checked = set()
        for case in office["cases"]:
            name = Path(case["input"]).stem.removesuffix("-preserved")
            fixture = evidence[name]
            output = args.office / case["output"]
            media = VECTORS.IMAGES.office_media(output, fixture["suffix"])
            assert DIGEST(media) == fixture["metafileSha256"], name
            actual = DECK.raster(args.candidate, output, args.output / f"{name}-office")
            assert DIGEST(actual.tobytes()) == fixture["candidateRgbSha256"], name
            fixture["officeReopenedExact"] = True
            checked.add(name)
        assert checked == set(evidence)
        evidence["powerPoint"] = office
    else:
        native = json.loads((args.native / "native-values.json").read_text(encoding="utf-8-sig"))
        wmf, wmf_points = wmf_scene(native["palettes"])
        names = ("blur-19_5", "blur-20", "blur-32", "blur-255", "alpha-3", "alpha-20", "brush-string")
        fixtures = [(name, ".emf", (args.native / f"{name}.emf").read_bytes()) for name in names]
        fixtures.extend([("palette-bitmap16", ".wmf", wmf), ("brush-indexed", ".emf", indexed_glyphs())])
        evidence = {}
        for name, suffix, payload in fixtures:
            original = args.output / f"{name}.pptx"
            SCENE.enlarged_shell(args.shells / f"{suffix[1:]}.pptx", original, payload, f"ppt/media/image1{suffix}")
            actual = DECK.raster(args.candidate, original, args.output / f"{name}-candidate")
            frozen = DECK.raster(args.frozen, original, args.output / f"{name}-frozen")
            frame = DECK.picture_frame(original, actual)
            baseline = {"suffix": suffix, "metafileSha256": DIGEST(payload),
                        "candidateRgbSha256": DIGEST(actual.tobytes()), "frozenRgbSha256": DIGEST(frozen.tobytes())}
            if name.startswith("brush-"):
                indexed = name.endswith("indexed")
                metrics = color_counts(actual, frame, payload, indexed=indexed)
                baseline["coverage"] = metrics
                print(name, metrics, flush=True)
                assert metrics[0]["over"] > 50 and metrics[0]["copy"] == 0
                assert metrics[1]["copy"] > 50 and metrics[1]["copyShades"] >= 8
                assert metrics[2]["clear"] > 35 and all(row["untouched"] > 100 for row in metrics)
                if not indexed:
                    with Image.open(args.native / f"{name}.png") as png:
                        composite = Image.alpha_composite(Image.new("RGBA", png.size, "white"), png.convert("RGBA"))
                        oracle = color_counts(composite.convert("RGB"), None, payload, native=True)
                    assert oracle[0]["over"] > 50 and oracle[1]["copy"] > 50 and oracle[2]["clear"] > 35
                    baseline["nativeCoverage"] = oracle
                    assert metrics[2]["copy"] > 10 and oracle[2]["copy"] > 10
            else:
                projected = False
                if name == "palette-bitmap16":
                    samples, limit = wmf_points, 0
                elif name.startswith("alpha-"):
                    raw = native["premultiplied"][name]
                    assert raw == [63, 36, 18, 64], raw
                    native_points = [(i, j) for i in (35, 50, 65) for j in (35, 50, 65)]
                    scaled = frame_points(payload, [(x + .5, y + .5) for x, y in native_points])
                    samples, limit = list(zip(scaled, [raw[:3]] * len(scaled), strict=True)), 0
                    baseline["nativePremultiplied"] = raw
                else:
                    projected = True
                    samples = blur_probes(payload, frame, args.native, name)
                    limit = 2
                if not projected:
                    samples = [(point, expected, [expected]) for point, expected in samples]
                checks = [{"point": point, "expected": expected, "candidate": SCENE.pixel(actual, frame, point),
                           "frozen": SCENE.pixel(frozen, frame, point)} for point, expected, _ in samples]
                errors = []
                for check, (_, _, texels) in zip(checks, samples, strict=True):
                    # Allow one source texel of subpixel phase at the downscaled PPTX boundary.
                    # The Rust filter tests separately verify kernel values by exact texel index.
                    low, high = list(map(min, zip(*texels))), list(map(max, zip(*texels)))
                    errors.append(max(max(l - c, c - h, 0) for c, l, h in zip(check["candidate"], low, high)))
                    if projected and low != high:
                        check["projectedTexelRange"] = [low, high]
                print(name, len(checks), "max error", max(errors), Counter(errors), flush=True)
                assert max(errors) <= limit, (name, [(check, error) for check, error in zip(checks, errors) if error > limit])
                baseline.update({"probes": checks, "maxChannelError": max(errors)})
            preserved = args.output / f"{name}-preserved.pptx"
            DECK.call(args.candidate, "exchange", original.resolve(), "--mode", "preserve", "--output", preserved.resolve())
            with ZipFile(original) as before, ZipFile(preserved) as after:
                assert sorted(before.namelist()) == sorted(after.namelist())
                assert all(before.read(part) == after.read(part) for part in before.namelist())
            reopened = DECK.raster(args.candidate, preserved, args.output / f"{name}-reopened")
            assert reopened.tobytes() == actual.tobytes()
            evidence[name] = baseline | {"preservePartsExact": True, "candidateReopenedExact": True}
    report.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
