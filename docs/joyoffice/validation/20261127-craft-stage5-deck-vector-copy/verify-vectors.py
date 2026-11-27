"""Compare JoyDeck vector compositing against Windows output and preserved PPTX parts."""

import argparse
import importlib.util
import json
from pathlib import Path
from struct import pack, unpack_from
from zipfile import ZipFile

from PIL import Image

PREVIOUS = Path(__file__).resolve().parents[1] / "20261127-craft-stage5-deck-image-composite/verify-images.py"
SPEC = importlib.util.spec_from_file_location("images", PREVIOUS)
IMAGES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMAGES)
SCENE, DECK, DIGEST = IMAGES.SCENE, IMAGES.DECK, IMAGES.DIGEST


def rop_scene():
    records = [
        (39, pack("<4I", 1, 0, 0x3355AA, 0)), (39, pack("<4I", 2, 0, 0xCC963C, 0)),
        (38, pack("<5I", 3, 0, 5, 0, 0x1AC3ED)), (37, pack("<I", 0x80000008)),
        (37, pack("<I", 1)), (43, pack("<4i", 0, 0, 100, 100)),
    ]
    probes = []
    for mode in range(1, 17):
        x, y = ((mode - 1) % 4 * 25, (mode - 1) // 4 * 25)
        records.extend([
            (33, b""), (20, pack("<I", mode)),
            (37, pack("<I", 2)), (37, pack("<I", 3)),
            (43, pack("<4i", x + 6, y + 6, x + 19, y + 19)),
            (34, pack("<i", -1)),
        ])
        probes.extend((x + dx, y + 12) for dx in (12, 7, 5, 2))
    return SCENE.wrap_emf(records), probes


def wmf_record(code, payload):
    return pack("<IH", (6 + len(payload)) // 2, code) + payload


def bitmap16(width, height, depth, pixel, reserved=False):
    stride = (width * depth + 15) // 16 * 2
    rows = bytearray()
    for y in range(height):
        row = bytearray()
        for x in range(width):
            value = pixel(x, y)
            if depth == 1:
                if x % 8 == 0:
                    row.append(0)
                row[-1] |= value << (7 - x % 8)
            elif depth == 16:
                row.extend(pack("<H", value))
            else:
                r, g, b = value
                row.extend((b, g, r))
                if depth == 32:
                    row.append(0x7F)
        rows.extend(row + bytes([0xA5]) * (stride - len(row)))
    return pack("<4h2B", 0, width, height, stride, 1, depth) + bytes(22 if reserved else 0) + rows


def wmf_scene():
    rop_copy = 0x00CC0020
    off, on = (170, 85, 51), (60, 150, 204)
    first, second, third, fourth = (255, 0, 0), (0, 255, 0), (0, 0, 255), (238, 188, 12)
    bmp = bitmap16(9, 8, 24, lambda x, y: (first, second, third, fourth)[2 * (y >= 4) + (x >= 4)])
    mono = bitmap16(9, 8, 1, lambda x, y: int((x >= 4) != (y >= 4)))
    rgb32 = bitmap16(8, 8, 32, lambda x, y: on)
    rgb16 = bitmap16(8, 8, 16, lambda x, y: 0x7C1F)
    pattern = bitmap16(8, 8, 1, lambda x, y: (x + y) % 2, reserved=True)
    records = [
        (0x0103, pack("<H", 8)), (0x020B, pack("<2h", 0, 0)), (0x020C, pack("<2h", 100, 100)),
        (0x020D, pack("<2h", 0, 0)), (0x020E, pack("<2h", 100, 100)),
        (0x02FC, pack("<HIH", 0, 0x3355AA, 0)), (0x012D, pack("<H", 0)),
        (0x012D, pack("<H", 0x8008)), (0x041B, pack("<4h", 100, 100, 0, 0)),
        # A cropped 24-bit scan. RecordFunction's high byte counts the fixed parameters.
        (0x0922, pack("<I6h", rop_copy, 2, 2, 6, 6, 8, 8) + bmp),
        # Stretch the same bitmap. The 27-byte pixel rows have word padding.
        (0x0B23, pack("<I8h", rop_copy, 8, 8, 0, 0, 26, 35, 5, 22) + bmp),
        # Black and white in a monochrome bitmap expand using the current DC colors.
        (0x0209, pack("<I", 0x0014E6)), (0x0201, pack("<I", 0xE64010)),
        (0x0B23, pack("<I8h", rop_copy, 8, 8, 0, 0, 24, 34, 6, 62) + mono),
        # Embedded BGRX ignores its fourth byte, and SRCINVERT reads the painted backdrop.
        (0x0B23, pack("<I8h", 0x00660046, 8, 8, 0, 0, 22, 25, 39, 8) + rgb32),
        (0x0B23, pack("<I8h", rop_copy, 8, 8, 0, 0, 22, 25, 39, 40) + rgb16),
        # Both bitmap-less fields include the reserved word before the destination fields.
        (0x0922, pack("<I7h", 0x00550009, 0, 0, 123, 21, 24, 39, 72)),
        (0x0B23, pack("<I9h", 0x00FF0062, 8, 8, 0, 0, 123, 14, 16, 70, 77)),
        # Bitmap16 pattern brush and a separate no-source PATCOPY transfer.
        (0x01F9, pattern), (0x012D, pack("<H", 1)),
        (0x0209, pack("<I", 0)), (0x0201, pack("<I", 0xFFFFFF)),
        (0x0922, pack("<I7h", 0x00F00021, 0, 0, 0, 18, 59, 70, 8)),
        (0x0000, b""),
    ]
    encoded = [wmf_record(*record) for record in records]
    data = pack("<HHHIHIH", 1, 9, 0x300, 9 + sum(map(len, encoded)) // 2,
                2, max(map(len, encoded)) // 2, 0) + b"".join(encoded)
    text, background = (230, 20, 0), (16, 64, 230)
    probes = [
        ((8.5, 8.5), first), ((11.5, 8.5), second), ((8.5, 11.5), third),
        ((12.5, 12.5), fourth), ((30, 11), first), ((48, 11), second),
        ((30, 24), third), ((48, 24), fourth), ((66, 10), text), ((88, 10), background),
        ((66, 24), background), ((88, 24), text),
        ((20, 50), tuple(a ^ b for a, b in zip(off, on))), ((53, 50), (255, 0, 255)),
        ((84, 50), tuple(255 - c for c in off)), ((84, 78), (255, 255, 255)), ((3, 50), off),
        ((16.5, 77.5), (255, 255, 255)), ((17.5, 77.5), (0, 0, 0)),
    ]
    return data, probes


def native_pixels(path, payload, coordinates):
    with Image.open(path) as image:
        rgba = image.convert("RGBA")
    visible = Image.alpha_composite(Image.new("RGBA", rgba.size, "white"), rgba).convert("RGB")
    # Recording accepts logical image coordinates, while the EMF frame is 0.01 mm.
    # Recover its device span from the recorded device and physical size.
    left, top, right, bottom = unpack_from("<4i", payload, 24)
    pixels_x, pixels_y, mm_x, mm_y = unpack_from("<4i", payload, 72)
    sx, sy = pixels_x / (100 * mm_x), pixels_y / (100 * mm_y)
    frame = left * sx, top * sy, (right - left) * sx, (bottom - top) * sy
    return [
        (((x - frame[0]) * 100 / frame[2], (y - frame[1]) * 100 / frame[3]), visible.getpixel(native))
        for (x, y), native in coordinates
    ]


def blur_points():
    points = []
    for (x, y), (nx, ny) in (
        ((22, 22), (21, 21)), ((71, 22), (70, 21)), ((22, 71), (21, 70)), ((71, 71), (72, 70))
    ):
        for delta in (-8, -6, -4, -2, 0, 2, 4, 6, 8):
            points.append(((x + delta, y), (nx + delta, ny)))
            if delta:
                points.append(((x, y + delta), (nx, ny + delta)))
    return points


def proximity(actual, expected):
    return max(abs(a - b) for a, b in zip(actual, expected, strict=True))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("frozen", "candidate", "shells", "native", "output"):
        p.add_argument(name, type=Path)
    p.add_argument("--office", type=Path)
    p.add_argument("--prepare-wmf", action="store_true")
    args = p.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    file = args.output / "verification.json"
    wmf, wmf_probes = wmf_scene()
    if args.prepare_wmf:
        args.native.mkdir(parents=True, exist_ok=True)
        (args.native / "bitmap16.wmf").write_bytes(wmf)
        (args.native / "wmf-points.json").write_text(json.dumps([[int(x), int(y)] for (x, y), _ in wmf_probes]))
        return
    if args.office:
        evidence = json.loads(file.read_text(encoding="utf-8"))
        office = json.loads((args.office / "verification.json").read_text(encoding="utf-8-sig"))
        checked = set()
        for case in office["cases"]:
            name = Path(case["input"]).stem.removesuffix("-preserved")
            output = args.office / case["output"]
            fixture = evidence[name]
            retained = IMAGES.office_media(output, fixture["suffix"])
            assert DIGEST(retained) == fixture["metafileSha256"], name
            actual = DECK.raster(args.candidate, output, args.output / f"{name}-office")
            assert DIGEST(actual.tobytes()) == fixture["candidateRgbSha256"], name
            fixture["officeReopenedExact"] = True
            checked.add(name)
        assert checked == {name for name, case in evidence.items() if "suffix" in case}
        evidence["powerPoint"] = office
    else:
        rop, points = rop_scene()
        gdi = json.loads((args.native / "rop2-windows.json").read_text(encoding="utf-8-sig"))
        gdi_values = [rgb for mode in gdi for rgb in mode]
        copy = (args.native / "copy.emf").read_bytes()
        blur = (args.native / "blur.emf").read_bytes()
        copy_points = [(12, 14), (15, 7), (34, 12), (60, 14), (60, 20), (12, 36),
                       (34, 36), (38, 45), (31, 60), (69, 60), (31, 82), (69, 82), (8, 65)]
        probes = native_pixels(args.native / "copy.png", copy, ((point, point) for point in copy_points))
        fixtures = [
            ("blur", ".emf", blur, native_pixels(args.native / "blur.png", blur, blur_points()), 0),
            ("copy", ".emf", copy, probes, 0),
            ("rop2", ".emf", rop, list(zip(points, gdi_values, strict=True)), 0),
            ("bitmap16", ".wmf", wmf, wmf_probes, 0),
        ]
        evidence = {}
        for name, suffix, payload, values, limit in fixtures:
            original = args.output / f"{name}.pptx"
            SCENE.enlarged_shell(args.shells / f"{suffix[1:]}.pptx", original, payload, f"ppt/media/image1{suffix}")
            candidate = DECK.raster(args.candidate, original, args.output / f"{name}-candidate")
            frozen = DECK.raster(args.frozen, original, args.output / f"{name}-frozen")
            frame = DECK.picture_frame(original, candidate)
            checks = []
            for point, expected in values:
                actual = SCENE.pixel(candidate, frame, point)
                record = {"point": point, "expected": expected, "candidate": actual,
                          "frozen": SCENE.pixel(frozen, frame, point)}
                assert proximity(actual, expected) <= limit, (name, record)
                checks.append(record)
            preserved = args.output / f"{name}-preserved.pptx"
            DECK.call(args.candidate, "exchange", original.resolve(), "--mode", "preserve",
                      "--output", preserved.resolve())
            with ZipFile(original) as before, ZipFile(preserved) as after:
                assert sorted(before.namelist()) == sorted(after.namelist())
                assert all(before.read(part) == after.read(part) for part in before.namelist())
            reopened = DECK.raster(args.candidate, preserved, args.output / f"{name}-reopened")
            assert reopened.tobytes() == candidate.tobytes()
            evidence[name] = {
                "suffix": suffix, "metafileSha256": DIGEST(payload), "candidateRgbSha256": DIGEST(candidate.tobytes()),
                "frozenRgbSha256": DIGEST(frozen.tobytes()), "preservePartsExact": True, "probes": checks,
            }
            print(f"{name} passed {len(checks)} probes")
        wmf_report = args.native / "wmf-windows.json"
        if wmf_report.exists():
            evidence["bitmap16"]["windowsProbes"] = json.loads(wmf_report.read_text(encoding="utf-8-sig"))
    file.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8", newline="\n")
    print("Passed:", ", ".join(evidence))


if __name__ == "__main__":
    main()
