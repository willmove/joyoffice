"""Compare JoyDeck shape, clock, random-bar and dissolve transitions with a PowerPoint movie."""

import argparse
import hashlib
import json
from pathlib import Path
import runpy
import statistics
import subprocess

masks = runpy.run_path(str(Path(__file__).with_name("check-soft-masks.py")))
motion = masks["motion"]
CASES = [
    ("none", ""),
    ("shape", "circle"),
    ("shape", "diamond"),
    ("shape", "plus"),
    ("shape", "in"),
    ("shape", "out"),
    ("clock", "clockwise"),
    ("clock", "wedge"),
    ("randomBar", "vert"),
    ("randomBar", "horz"),
    ("dissolve", ""),
    ("clock", "counterClockwise"),
    ("shape", "circle"),
]
NOISE = {"randomBar", "dissolve"}


def sample(bitmap, slide, kind, colors):
    if kind not in NOISE:
        return bytes(
            round(v * 255) for v in masks["coverage"](bitmap, slide, colors)
        ).hex()
    # Random seeds differ between renderers. Measure total revealed area and
    # spatial variation parallel/perpendicular to the strips at the same resolution.
    new, old = colors[slide % 2], colors[(slide + 1) % 2]
    delta = [a - b for a, b in zip(new, old)]
    norm = sum(v * v for v in delta)
    values = [
        max(
            0, min(1, sum((bitmap[i + c] - old[c]) * delta[c] for c in range(3)) / norm)
        )
        for i in range(0, len(bitmap), 3)
    ]
    row = [statistics.mean(values[y * 96 : (y + 1) * 96]) for y in range(54)]
    col = [statistics.mean(values[x::96]) for x in range(96)]
    return [
        round(v, 6)
        for v in (
            statistics.mean(values),
            statistics.pstdev(row),
            statistics.pstdev(col),
        )
    ]


def measure(movie, deck, raw):
    data = (
        raw.read_bytes()
        if raw
        else subprocess.check_output(
            [
                "ffmpeg",
                "-v",
                "error",
                "-i",
                movie,
                "-vf",
                "scale=96:54:flags=area",
                "-vsync",
                "0",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "-",
            ]
        )
    )
    stride = 96 * 54 * 3
    assert len(data) == stride * 912

    def frame(i):
        return data[i * stride : (i + 1) * stride]

    colors = [frame(i)[:3] for i in (8, 80)]
    rows = []
    for slide, (kind, option) in enumerate(CASES[1:], 1):
        for j in range(1, 16):
            rows.append(
                [
                    slide + 1,
                    kind,
                    option,
                    j / 16,
                    sample(frame(25 + (slide - 1) * 74 + j * 3), slide, kind, colors),
                ]
            )
    return {
        "source": "Microsoft PowerPoint 16.115 for macOS, 1920x1080 movie, 1600 ms per transition, 767 ms advance",
        "movieSha256": hashlib.sha256(movie.read_bytes()).hexdigest(),
        "packagePayloadSha256": masks["payloads"](deck),
        "frames": 912,
        "size": [96, 54],
        "pixelIndices": masks["SAMPLES"],
        "samples": rows,
    }


def verify(reference, old, new, out):
    out.mkdir(parents=True)
    original = out / "几何与图案.pptx"
    motion["generate"](original, CASES)
    assert masks["payloads"](original) == reference["packagePayloadSha256"]
    source = out / "sampling.pptx"
    motion["hold_for_sampling"](original, source, len(CASES))
    result = {}
    rows = reference["samples"]
    for label, cli in [("baseline", old), ("candidate", new)]:
        cases = {}
        spatial = []
        means = []
        variability = []
        frames = motion["sample_cli"](cli, source, rows, out / label)
        for (slide, kind, opt, _, expected), rgb in zip(rows, frames, strict=True):
            measured = sample(rgb, slide - 1, kind, masks["SOURCE"])
            if kind in NOISE:
                errors = [abs(a - b) for a, b in zip(measured, expected, strict=True)]
                means.append(errors[0])
                variability.extend(errors[1:])
                diff = errors[:1]
            else:
                diff = [
                    abs(a - b) / 255
                    for a, b in zip(
                        bytes.fromhex(measured), bytes.fromhex(expected), strict=True
                    )
                ]
                spatial.extend(diff)
            cases.setdefault(kind + "/" + opt, []).extend(diff)
        result[label] = {
            "geometricPixelErrors": masks["summarize"](spatial),
            "randomCoverageErrors": masks["summarize"](means),
            "randomAxisVariationErrors": masks["summarize"](variability),
            "perOption": {k: masks["summarize"](v) for k, v in cases.items()},
        }
    (out / "report.json").write_text(json.dumps(result, indent=2) + "\n")
    for metric in ["geometricPixelErrors", "randomCoverageErrors"]:
        assert (
            result["candidate"][metric]["meanAbsoluteError"]
            < result["baseline"][metric]["meanAbsoluteError"]
        ), metric


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="mode", required=True)
    for command, names in [
        ("generate", ["input"]),
        ("measure", ["movie", "input", "output"]),
        ("verify", ["reference", "old", "new", "output"]),
    ]:
        child = commands.add_parser(command)
        for name in names:
            child.add_argument(name, type=Path)
        if command == "measure":
            child.add_argument("--rgb96x54", type=Path)
    args = parser.parse_args()
    if args.mode == "generate":
        motion["generate"](args.input, CASES)
    elif args.mode == "measure":
        args.output.write_text(
            json.dumps(measure(args.movie, args.input, args.rgb96x54), indent=2) + "\n"
        )
    else:
        verify(
            json.loads(args.reference.read_text()),
            args.old.resolve(),
            args.new.resolve(),
            args.output.resolve(),
        )
