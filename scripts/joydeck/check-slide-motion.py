"""Generate a timed slide-motion study and measure its independent PowerPoint video export."""

import argparse
import hashlib
import itertools
import json
import math
import shutil
import re
import runpy
import statistics
import subprocess
import tempfile
import xml.etree.ElementTree as E
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED

window = runpy.run_path(str(Path(__file__).with_name("check-transition-window.py")))

P = "http://schemas.openxmlformats.org/presentationml/2006/main"
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
REL = "http://schemas.openxmlformats.org/package/2006/relationships"
CT = "http://schemas.openxmlformats.org/package/2006/content-types"
P14 = "http://schemas.microsoft.com/office/powerpoint/2010/main"
for k, v in [("p", P), ("a", A), ("r", R), ("p14", P14)]:
    E.register_namespace(k, v)
CASES = [
    ("none", ""),
    ("cut", ""),
    ("cut", "throughBlack"),
    ("fade", "smoothly"),
    ("fade", "throughBlack"),
] + [
    (kind, option)
    for kind in ("push", "cover", "uncover")
    for option in (
        ("l", "r", "u", "d")
        if kind == "push"
        else ("l", "r", "u", "d", "lu", "ru", "ld", "rd")
    )
]


def generate(out, cases=CASES):
    with tempfile.TemporaryDirectory() as folder:
        seed = Path(folder) / "seed.pptx"
        window["deck"](seed)
        extend(seed, out, cases)


def extend(seed, out, cases=CASES):
    with ZipFile(seed) as z, ZipFile(out, "x", ZIP_DEFLATED) as w:
        for name in z.namelist():
            if re.fullmatch(r"ppt/slides/(?:_rels/)?slide\d+\.xml(?:\.rels)?", name):
                continue
            data = z.read(name)
            if name in (
                "[Content_Types].xml",
                "ppt/presentation.xml",
                "ppt/_rels/presentation.xml.rels",
            ):
                xml = E.fromstring(data)
                if name == "ppt/presentation.xml":
                    slides = xml.find(f"{{{P}}}sldIdLst")
                    slides.clear()
                    for i in range(1, len(cases) + 1):
                        E.SubElement(
                            slides,
                            f"{{{P}}}sldId",
                            {"id": str(300 + i), f"{{{R}}}id": f"rIdMotion{i}"},
                        )
                elif name.startswith("ppt/"):
                    for rel in list(xml):
                        if rel.get("Type", "").endswith("/slide"):
                            xml.remove(rel)
                    for i in range(1, len(cases) + 1):
                        E.SubElement(
                            xml,
                            f"{{{REL}}}Relationship",
                            Id=f"rIdMotion{i}",
                            Type=R + "/slide",
                            Target=f"slides/slide{i}.xml",
                        )
                else:
                    for part in list(xml):
                        if re.fullmatch(
                            r"/ppt/slides/slide\d+\.xml", part.get("PartName", "")
                        ):
                            xml.remove(part)
                    for i in range(1, len(cases) + 1):
                        E.SubElement(
                            xml,
                            f"{{{CT}}}Override",
                            PartName=f"/ppt/slides/slide{i}.xml",
                            ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
                        )
                data = E.tostring(xml, encoding="utf-8", xml_declaration=True)
            w.writestr(z.getinfo(name), data)
        rels = E.fromstring(z.read("ppt/slides/_rels/slide1.xml.rels"))
        for item in list(rels):
            if not item.get("Type", "").endswith("/slideLayout"):
                rels.remove(item)
        for idx, (kind, option) in enumerate(cases, 1):
            xml = E.fromstring(z.read("ppt/slides/slide1.xml"))
            xml.find(".//{*}bg/{*}bgPr/{*}solidFill/{*}srgbClr").set(
                "val", "E82820" if idx % 2 else "1848D8"
            )
            tr = E.SubElement(
                xml,
                f"{{{P}}}transition",
                {"advTm": "767", "advClick": "0", f"{{{P14}}}dur": "1600"},
            )
            if kind != "none":
                tag, attrs = window["effect_xml"](kind, option)
                E.SubElement(tr, f"{{{P}}}" + tag, attrs)
            w.writestr(
                f"ppt/slides/slide{idx}.xml",
                E.tostring(xml, encoding="utf-8", xml_declaration=True),
            )
            w.writestr(
                f"ppt/slides/_rels/slide{idx}.xml.rels",
                E.tostring(rels, encoding="utf-8", xml_declaration=True),
            )


def metrics(bitmap, index, kind, direction):
    red = index % 2 == 0
    hot = [(r > b) == red for r, _, b in zip(*[iter(bitmap)] * 3)]
    count = sum(hot)
    fraction = count / len(hot)
    if kind == "cover" and len(direction) == 2:
        distance = math.sqrt(fraction)
    elif kind == "uncover" and len(direction) == 2:
        distance = 1 - math.sqrt(1 - fraction)
    else:
        distance = fraction
    cx = (
        sum((i % 96 + 0.5) / 96 for i, p in enumerate(hot) if p) / count
        if count
        else 0.5
    )
    cy = (
        sum((i // 96 + 0.5) / 54 for i, p in enumerate(hot) if p) / count
        if count
        else 0.5
    )
    rgb = [round(statistics.mean(bitmap[c::3]), 3) for c in range(3)]
    return distance, cx, cy, rgb


def measure(movie, output):
    # Decode presentation-order frames without rate conversion. Keep the movie's
    # timestamps since the QuickTime container uses a 600-unit time base.
    ts = subprocess.check_output(
        [
            "ffprobe",
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "frame=best_effort_timestamp_time",
            "-of",
            "csv=p=0",
            movie,
        ],
        text=True,
    )
    times = [
        float(t.strip().strip(",")) for t in ts.splitlines() if t.strip().strip(",")
    ]
    width, height = 96, 54
    raw = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-i",
            movie,
            "-vf",
            f"scale={width}:{height}:flags=area",
            "-vsync",
            "0",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]
    )
    size = width * height * 3
    if len(times) != 1752 or len(raw) != len(times) * size:
        raise ValueError(
            "unexpected export frame count; recalibrate the slide boundaries for this export"
        )
    observations = []
    for i, (kind, direction) in enumerate(CASES):
        # This 30 fps export advances after 25 held frames. A normal cut and the
        # through-black cut each use 50 frames, regular 1.6 s transitions use
        # 49 moving frames plus 25 held frames. Midpoint sampling skips codec edges.
        start = [0, 25, 75][i] if i < 3 else 125 + (i - 3) * 74
        if i == 0:
            continue
        for step in range(1, 16):
            j = round(start + 48 * step / 16)
            distance, x, y, rgb = metrics(
                raw[j * size : (j + 1) * size], i, kind, direction
            )
            observations.append(
                [
                    i + 1,
                    kind,
                    direction,
                    step / 16,
                    round(times[j], 6),
                    round(distance, 6),
                    round(x, 6),
                    round(y, 6),
                    rgb,
                ]
            )
    data = {
        "source": "Microsoft PowerPoint 16.115 for macOS, H.264 QuickTime export of the generated 25-slide deck. Each slide advances after 767 ms; the declared transition duration is 1600 ms.",
        "movieSha256": hashlib.sha256(movie.read_bytes()).hexdigest(),
        "frames": len(times),
        "solidRgb": [
            metrics(raw[n * size : (n + 1) * size], 0, "none", "")[3] for n in (10, 40)
        ],
        "columns": [
            "page",
            "effect",
            "option",
            "progress",
            "movieSeconds",
            "incomingDisplacement",
            "incomingCentroidX",
            "incomingCentroidY",
            "meanRgb",
        ],
        "observations": observations,
    }
    output.write_text(
        json.dumps(data, separators=(",", ":"), ensure_ascii=False) + "\n"
    )


def decoded(directory, count):
    try:
        from PIL import Image
    except ImportError:
        data = subprocess.check_output(
            [
                "ffmpeg",
                "-v",
                "error",
                "-start_number",
                "0",
                "-i",
                str(directory / "frame-%04d.png"),
                "-vf",
                "scale=96:54:flags=area",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ]
        )
        stride = 96 * 54 * 3
        assert len(data) == count * stride
        return [data[i * stride : (i + 1) * stride] for i in range(count)]
    frames = []
    for i in range(count):
        with Image.open(directory / ("frame-%04d.png" % i)) as image:
            frames.append(
                image.convert("RGB").resize((96, 54), Image.Resampling.BOX).tobytes()
            )
    return frames


def sample_cli(cli, doc, observations, directory):
    directory.mkdir()
    image_index = 0
    for page, samples in itertools.groupby(observations, key=lambda row: row[0]):
        # The control session has a bounded replay ledger. One session per
        # transition also proves each page can be reached from a fresh reader.
        with window["app"](
            cli,
            ["control", "--root", str(doc.parent)],
            directory,
            {},
            f"page-{page}.log",
        ) as proc:
            client = window["DeckControl"](proc)
            opened = client.request("document.open", params={"path": str(doc)})
            guard = {"target": opened["documentId"], "expectedRevision": 0}
            client.request("show.start", params={"page": page - 1}, **guard)
            client.request("show.action", params={"action": "next"}, **guard)
            previous_time = 0.0
            for _, _, _, progress, *_ in samples:
                current = progress * 1.6
                position = client.request(
                    "show.tick", params={"seconds": current - previous_time}, **guard
                )
                assert position["page"] == page and not position["ended"], position
                client.request(
                    "show.frame",
                    params={
                        "path": str(directory / ("frame-%04d.png" % image_index)),
                        "scale": 0.1,
                    },
                    **guard,
                )
                previous_time = current
                image_index += 1
    return decoded(directory, image_index)


def amount(rgb, kind, option, progress, old, new):
    if (kind == "cut" or kind == "fade") and option == "throughBlack":
        base = old if progress < 0.5 else new
        return sum(c * b for c, b in zip(rgb, base)) / sum(b * b for b in base)
    delta = [b - a for a, b in zip(old, new)]
    return sum((c - a) * d for c, a, d in zip(rgb, old, delta)) / sum(
        d * d for d in delta
    )


def summary(values):
    a = sorted(values)
    return {
        "samples": len(a),
        "meanAbsError": round(statistics.mean(a), 6),
        "p95AbsError": round(a[math.ceil(0.95 * len(a)) - 1], 6),
        "maximumAbsError": round(max(a), 6),
    }


def hold_for_sampling(source, destination, count_expected=len(CASES)):
    # Older players count the transition itself as hold time. Lengthen only
    # the hold in both visual comparisons to keep those clocks on the same page.
    count = 0
    with ZipFile(source) as src, ZipFile(destination, "x", ZIP_DEFLATED) as dst:
        for info in src.infolist():
            raw = src.read(info)
            if re.fullmatch(r"ppt/slides/slide\d+\.xml", info.filename):
                assert raw.count(b'advTm="767"') == 1
                raw = raw.replace(b'advTm="767"', b'advTm="60000"')
                count += 1
            dst.writestr(info, raw)
    assert count == count_expected


def timed_pages(cli, doc, directory):
    # Exercise the unmodified input at either side of the documented hold
    # deadline. Cover all ordinary-duration transitions, including the last page.
    positions = {}
    for page in range(4, len(CASES) + 1):
        with window["app"](
            cli,
            ["control", "--root", str(doc.parent)],
            directory,
            {},
            f"clock-{page}.log",
        ) as proc:
            client = window["DeckControl"](proc)
            opened = client.request("document.open", params={"path": str(doc)})
            guard = {"target": opened["documentId"], "expectedRevision": 0}
            client.request("show.start", params={"page": page - 1}, **guard)
            client.request("show.action", params={"action": "next"}, **guard)
            at = []
            for dt in (0.8, 0.8, 0.766, 0.002):
                pos = client.request("show.tick", params={"seconds": dt}, **guard)
                at.append(None if pos["ended"] else pos["page"])
            positions[page] = at
    return positions


def verify(source, measurements, old, candidate, output):
    output = output.resolve()
    output.mkdir(parents=True)
    timed = output / "input.pptx"
    shutil.copyfile(source, timed)
    doc = output / "extended-hold.pptx"
    hold_for_sampling(timed, doc)
    reference = json.loads(measurements.read_text())
    observations = reference["observations"]
    report = {}
    for version, cli in [("baseline", old), ("candidate", candidate)]:
        rows = sample_cli(cli.resolve(), doc, observations, output / version)
        groups = {}
        wrong_direction = []
        for row, frame in zip(observations, rows, strict=True):
            page, kind, option, t, _, expected, cx, cy, rgb = row
            i = page - 1
            actual, x, y, color = metrics(frame, i, kind, option)
            if kind in ("cut", "fade"):
                before, after = (
                    reference["solidRgb"][page % 2],
                    reference["solidRgb"][i % 2],
                )
                expected = amount(rgb, kind, option, t, before, after)
                colors = [[232, 40, 32], [24, 72, 216]]
                actual = amount(color, kind, option, t, colors[page % 2], colors[i % 2])
            else:
                if 0.10 < expected < 0.90:
                    for axis, at, want in [("x", x, cx), ("y", y, cy)]:
                        if abs(want - 0.5) > 0.08 and (at - 0.5) * (want - 0.5) < 0:
                            wrong_direction.append(
                                [page, kind, option, t, axis, at, want]
                            )
            key = kind + ("-diagonal" if len(option) == 2 else "")
            groups.setdefault(key, []).append(abs(expected - actual))
            if kind in ("cover", "uncover") and option not in ("l", "lu"):
                groups.setdefault("heldOutDirections", []).append(
                    abs(expected - actual)
                )
        clock = timed_pages(cli.resolve(), timed, output / version)
        early = sum(at[:3] != [page] * 3 for page, at in clock.items())
        if version == "candidate":
            assert not early, clock
            assert all(
                at[-1] == (page + 1 if page < len(CASES) else None)
                for page, at in clock.items()
            ), clock
        report[version] = {
            "automaticAdvance": {
                "timesSeconds": [0.8, 1.6, 2.366, 2.368],
                "pagesAdvancedEarly": early,
                "pages": clock,
            },
            "cliSha256": hashlib.sha256(cli.read_bytes()).hexdigest(),
            "wrongDirections": wrong_direction,
            "errors": {k: summary(v) for k, v in groups.items()},
        }
        assert not wrong_direction, wrong_direction[:4]
    for group, budget in [
        ("cut", 0.02),
        ("fade", 0.08),
        ("push", 0.05),
        ("cover", 0.10),
        ("cover-diagonal", 0.07),
        ("uncover", 0.10),
        ("uncover-diagonal", 0.08),
    ]:
        assert report["candidate"]["errors"][group]["maximumAbsError"] <= budget, group
    held = report["candidate"]["errors"]["heldOutDirections"]
    assert (
        held["samples"] == 180
        and held["meanAbsError"] <= 0.02
        and held["p95AbsError"] <= 0.07
    )
    assert (
        held["meanAbsError"]
        <= report["baseline"]["errors"]["heldOutDirections"]["meanAbsError"]
    )
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    g = commands.add_parser("generate")
    g.add_argument("output", type=Path)
    m = commands.add_parser("measure")
    m.add_argument("movie", type=Path)
    m.add_argument("output", type=Path)
    v = commands.add_parser("verify")
    for name in ["source", "measurements", "baseline", "candidate", "output"]:
        v.add_argument(name, type=Path)
    args = parser.parse_args()
    if args.command == "generate":
        generate(args.output)
    elif args.command == "measure":
        measure(args.movie, args.output)
    else:
        verify(
            args.source, args.measurements, args.baseline, args.candidate, args.output
        )


if __name__ == "__main__":
    main()
