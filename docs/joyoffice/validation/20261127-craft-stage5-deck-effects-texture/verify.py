"""Compare native GDI+ effects and textured glyphs through JoyDeck's PPTX renderer."""

import argparse
import importlib.util
import json
from pathlib import Path
from struct import pack
from zipfile import ZipFile

from PIL import Image

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-filter-palette/verify.py"
SPEC = importlib.util.spec_from_file_location("palette", PREVIOUS)
PALETTE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PALETTE)
SCENE, DECK, DIGEST = PALETTE.SCENE, PALETTE.DECK, PALETTE.DIGEST
IMAGES = PALETTE.VECTORS.IMAGES

GUIDS = {
    "brightness": "D3A1DBE1-8EC4-4C17-9F4C-EA97AD1C343D",
    "balance": "537E597D-251E-48DA-9664-29CA496B70F8",
    "curve": "DD6A0022-58E4-4A67-9D9B-D48EB881A53D",
    "hsl": "8B2DD6C3-EB07-4D87-A5F0-7108E26A9C5F",
    "levels": "99C354EC-2A31-4F3A-8C34-17A803B33A25",
    "redeye": "74D29D05-69A4-4266-9549-3CC52836B632",
    "sharpen": "63CBF3EE-C526-402C-8F71-62C540BF5142",
    "tint": "1077AF00-2848-4441-9489-44AD4C2D7A2C",
}


def rgba(value):
    return tuple((value >> shift) & 255 for shift in (16, 8, 0, 24))


def visible(pixel):
    r, g, b, a = pixel
    return tuple((a * c + (255 - a) * 255 + 127) // 255 for c in (r, g, b))


def delta(expected, actual):
    return max(abs(a - b) for a, b in zip(expected, actual, strict=True))


def draw_effect(records, name, parameters, bitmap, size, dest):
    w, h = size
    records.extend((
        IMAGES.bitmap(width=w, height=h, color_at=lambda x, y: bitmap[y * w + x] & 0xffffffff),
        IMAGES.effect(GUIDS[name], parameters),
        IMAGES.draw(dest, (0, 0, w, h), with_effect=True),
    ))


def tone_cases(native):
    for name, variants in (
        ("tone-levels", [
            ("combined_bc", "brightness", (30, 35), True, 1),
            ("combined_bc_neg", "brightness", (-30, 35), True, 1),
            ("combined_bc_flat", "brightness", (30, -35), True, 1),
            ("levels_combined", "levels", (85, 20, 10), True, 2),
            ("highlight_50", "levels", (50, 0, 0), True, 1),
            ("shadow_15", "levels", (100, 0, 15), True, 1),
            ("midtone_-30", "levels", (100, -30, 0), True, 1),
            ("midtone_75", "levels", (100, 75, 0), True, 2),
            ("balance_mixed", "balance", (21, -35, 62), True, 1),
            ("hsl_mixed", "hsl", (73, 31, -23), True, 14),
            ("tint_30", "tint", (0, 30), False, 2),
            ("tint_-30", "tint", (0, -30), False, 1),
        ]),
        ("tone-curves", [
            (f"curve_{mode}_{amount}", "curve", (mode, 0, amount), True, 7 if mode in (2, 3, 4) else 2)
            for mode, amount in ((0, -50), (1, 150), (2, -30), (2, 60), (3, -30), (3, 60),
                                 (4, -60), (4, 30), (5, 50), (6, 50), (7, 150))
        ] + [("curve_single", "curve", (0, 2, 31), True, 2)]),
    ):
        records = [SCENE.fill(SCENE.WHITE), SCENE.plus(0x4021, 5)]
        points = []
        indices = (5, 16, 32, 61, 73, 96, 111, 126, 129, 144, 158, 181, 203, 225, 247)
        for index, (key, effect, parameters, chroma, tolerance) in enumerate(variants):
            rows = 3 if chroma else 1
            source = [256 * y + x for y in range(rows) for x in indices]
            swatches = [native["inputs"]["ramp"][i] for i in source]
            x, y = 3 + index % 3 * 33, 4 + index // 3 * 24
            draw_effect(records, effect, pack(f"<{len(parameters)}i", *parameters),
                        swatches, (len(indices), rows), (x, y, 30, 18))
            for i, src in enumerate(source):
                point = (x + 2 * (i % len(indices) + .5), y + 18 * (i // len(indices) + .5) / rows)
                points.append((key, point, visible(rgba(native[key][2][src])), tolerance))
        yield name, SCENE.emfplus(*records), points


def spatial_cases(native):
    for label, variants, source, size in (
        ("sharpen", [
            ("sharp_0.5", (0.5, 60)), ("sharp_1", (1, 60)), ("sharp_2", (2, 60)),
            ("sharp_3_25", (3, 25)), ("sharp_3", (3, 60)), ("sharp_3_95", (3, 95)),
            ("sharp_6", (6, 60)), ("sharp_20", (20, 60)), ("sharp_32", (32, 60)),
        ], "spatial", 61),
        ("red-eye", [("pupils", (2, 11, 22, 32, 43, 35, 22, 56, 43)),
                     ("pupils_offcenter", (1, 5, 17, 30, 45))], "pupils", 64),
    ):
        records = [SCENE.fill(SCENE.WHITE), SCENE.plus(0x4021, 5)]
        probes = []
        for i, (key, params) in enumerate(variants):
            x, y = (3 + i % 3 * 33, 4 + i // 3 * 31) if label == "sharpen" else (4 + i * 48, 19)
            # Fractional device extents keep the native subsampling grid out of the filter-only check.
            extent = 29.25 if label == "sharpen" else 44.25
            if label == "sharpen":
                positions = [(0, 0), (9, 5), (10, 5), (26, 25), (28, 25), (29, 25), (30, 25), (32, 25),
                             (25, 28), (25, 30), (29, 30), (31, 30), (49, 55)]
                effect, parameters, tolerance = label, pack("<2f", *params), 3
            else:
                positions = [(21, 32), (45, 32), (18, 32), (25, 32), (21, 29), (21, 28), (32, 32), (8, 8)]
                effect, parameters = "redeye", pack(f"<{len(params)}i", *params)
                tolerance = 4 if key == "pupils" else 16
            draw_effect(records, effect, parameters, native["inputs"][source], (size, size),
                        (x, y, extent, extent))
            for px, py in positions:
                point = (x + extent * (px + .5) / size, y + extent * (py + .5) / size)
                expected = visible(rgba(native[key][2][py * size + px]))
                probes.append((key, point, expected, tolerance))
        yield label, SCENE.emfplus(*records), probes


def native_probes(folder, name, payload):
    with Image.open(folder / f"{name}.png") as png:
        reference = png.convert("RGBA")
    if name == "expanded":
        result = []
        for x, y, radius, expanded in ((12, 14, 3, False), (58, 14, 20, False),
                                       (12, 60, 3, True), (58, 60, 20, True)):
            positions = [(0, 0), (1, 10), (5, 10), (10, 0), (10, 8), (20, 10), (2, 3)]
            positions += [(-1, 10), (-2, 10), (22, 10), (11, -1), (11, -4), (-1, -1), (23, 10)]
            for dx, dy in positions:
                result.append((f"radius-{radius}-window-{expanded}", (x + dx, y + dy), 5, .4))
    else:
        result = []
        for i, mode in enumerate(("nearest", "bilinear", "bicubic", "high-bilinear", "high-bicubic")):
            for x, kind in ((7, "raw"), (40, "blur")):
                for dx in (3, 5, 7, 8, 12, 14, 16):
                    result.append((f"{mode}-{kind}", (x + dx, 8 + 19 * i), 5, .13))
                if mode.startswith("high-"):
                    for dx in (9, 10, 11):
                        limit = 7 if mode == "high-bilinear" else 16
                        result.append((f"{mode}-{kind}-transition", (x + dx, 8 + 19 * i), limit, .35))
    probes = []
    for label, (x, y), tolerance, phase in result:
        point, = PALETTE.frame_points(payload, [(x + phase, y + phase)])
        probes.append((label, point, visible(reference.getpixel((x, y))), tolerance))
    return probes


def glyph_colors(image, lookup):
    outcomes = []
    for top in (6, 36, 66):
        rgb = [lookup(image, x, y) for y in range(top, top + 21) for x in range(5, 85)]
        outcomes.append({
            "background": rgb.count((0, 255, 0)),
            "redOver": sum(r >= 100 and g <= 155 and b <= 2 for r, g, b in rgb),
            "redCopy": sum(r >= 248 and 98 <= g <= 170 and abs(b - g) <= 3 for r, g, b in rgb),
            "blue": sum(b >= 218 and r <= 3 and g <= 15 for r, g, b in rgb),
            "clear": rgb.count((255, 255, 255)),
        })
    return outcomes


def glyph_probes(folder, payload, actual, frame):
    with Image.open(folder / "texture-glyphs.png") as png:
        reference = Image.alpha_composite(Image.new("RGBA", png.size, "white"), png.convert("RGBA")).convert("RGB")
    native = glyph_colors(reference, lambda im, x, y: im.getpixel((x, y)))

    def output(im, x, y):
        point, = PALETTE.frame_points(payload, [(x + .5, y + .5)])
        return SCENE.pixel(im, frame, point)

    candidate = glyph_colors(actual, output)
    for label, metrics in (("GDI+", native), ("JoyDeck", candidate)):
        over, flip, clamp = metrics
        assert over["redOver"] >= 35 and over["blue"] >= 25 and over["clear"] == 0, (label, over)
        assert flip["redCopy"] >= 35 and flip["blue"] >= 25 and flip["clear"] >= 65, (label, flip)
        assert clamp["clear"] >= 125 and clamp["blue"] == clamp["redCopy"] == 0, (label, clamp)
        assert all(row["background"] > 900 for row in metrics), (label, metrics)
    return {"native": native, "candidate": candidate}


def verify_office(args, report):
    evidence = json.loads(report.read_text(encoding="utf-8"))
    office = json.loads((args.office / "verification.json").read_text(encoding="utf-8-sig"))
    names = set()
    for case in office["cases"]:
        name = Path(case["input"]).stem.removesuffix("-preserved")
        slide = args.office / case["output"]
        assert DIGEST(IMAGES.office_media(slide, ".emf")) == evidence[name]["metafileSha256"], name
        actual = DECK.raster(args.candidate, slide, args.output / f"{name}-office")
        assert DIGEST(actual.tobytes()) == evidence[name]["candidateRgbSha256"], name
        evidence[name]["officeReopenedExact"] = True
        names.add(name)
    assert names == set(evidence)
    evidence["powerPoint"] = office
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("frozen", "candidate", "shells", "native", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--office", type=Path)
    parser.add_argument("--measure", action="store_true", help="Print numerical deviations without applying gates")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    report = args.output / "verification.json"
    if args.office:
        evidence = verify_office(args, report)
    else:
        native = json.loads((args.native / "native-effects.json").read_text(encoding="utf-8-sig"))
        native_names = ("expanded", "downsample", "texture-glyphs")
        native_files = [(name, (args.native / f"{name}.emf").read_bytes()) for name in native_names]
        fixtures = list(tone_cases(native)) + list(spatial_cases(native))
        fixtures += [(name, payload, [] if name == "texture-glyphs" else native_probes(args.native, name, payload))
                     for name, payload in native_files]
        evidence = {}
        for name, payload, probes in fixtures:
            original = args.output / f"{name}.pptx"
            SCENE.enlarged_shell(args.shells / "emf.pptx", original, payload, "ppt/media/image1.emf")
            actual = DECK.raster(args.candidate, original, args.output / f"{name}-candidate")
            frozen = DECK.raster(args.frozen, original, args.output / f"{name}-frozen")
            frame = DECK.picture_frame(original, actual)
            entry = {"metafileSha256": DIGEST(payload), "candidateRgbSha256": DIGEST(actual.tobytes()),
                     "frozenRgbSha256": DIGEST(frozen.tobytes())}
            if name == "texture-glyphs":
                colors = glyph_probes(args.native, payload, actual, frame)
                entry["coverage"] = colors
                print(name, colors, flush=True)
            else:
                checks, maxima = [], {}
                for effect, point, expected, limit in probes:
                    rgb = SCENE.pixel(actual, frame, point)
                    error = delta(expected, rgb)
                    record = {"effect": effect, "point": point, "expected": expected, "candidate": rgb, "limit": limit,
                              "frozen": SCENE.pixel(frozen, frame, point)}
                    if not args.measure:
                        assert error <= limit, (name, record, error)
                    maxima[effect] = max(error, maxima.get(effect, 0))
                    checks.append(record)
                entry.update({"probes": checks, "maxErrorByEffect": maxima})
                print(name, len(checks), maxima, flush=True)
            preserved = args.output / f"{name}-preserved.pptx"
            DECK.call(args.candidate, "exchange", original.resolve(), "--mode", "preserve",
                      "--output", preserved.resolve())
            with ZipFile(original) as before, ZipFile(preserved) as after:
                assert sorted(before.namelist()) == sorted(after.namelist())
                assert all(before.read(part) == after.read(part) for part in before.namelist())
            reopened = DECK.raster(args.candidate, preserved, args.output / f"{name}-reopened")
            assert reopened.tobytes() == actual.tobytes()
            evidence[name] = entry | {"preservePartsExact": True, "candidateReopenedExact": True}
    report.write_text(json.dumps(evidence, separators=(",", ":")) + "\n", encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
