"""Measure PowerPoint wipe/split coverage and compare both JoyDeck readers at the same times."""

import argparse
import hashlib
import json
import runpy
import statistics
import subprocess
from pathlib import Path
from zipfile import ZipFile

motion = runpy.run_path(str(Path(__file__).with_name("check-slide-motion.py")))
CASES = (
    [("none", "")]
    + [("wipe", o) for o in ("r", "l", "u", "d", "rd", "ru", "ld", "lu")]
    + [("split", o) for o in ("horzOut", "horzIn", "vertOut", "vertIn")]
)
SAMPLES = [96 * y + x for y in range(4, 54, 9) for x in range(4, 96, 8)]
SOURCE = [bytes.fromhex(c) for c in ("E82820", "1848D8")]


def payloads(file):
    with ZipFile(file) as archive:
        return hashlib.sha256(
            b"".join(
                n.encode() + b"\0" + hashlib.sha256(archive.read(n)).digest()
                for n in sorted(archive.namelist())
            )
        ).hexdigest()


def coverage(bitmap, slide, colors):
    new, old = colors[slide % 2], colors[(slide + 1) % 2]
    delta = [a - b for a, b in zip(new, old)]
    norm = sum(x * x for x in delta)
    return [
        max(
            0,
            min(
                1, sum((bitmap[3 * i + k] - old[k]) * delta[k] for k in range(3)) / norm
            ),
        )
        for i in SAMPLES
    ]


def measure(movie, source, raw=None):
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
    assert len(data) == 912 * stride, "unexpected movie timing; check frame boundaries"

    def frame(n):
        return data[n * stride : (n + 1) * stride]

    colors = [frame(j)[:3] for j in (8, 80)]
    observations = []
    for page, (kind, option) in enumerate(CASES[1:], 1):
        for step in range(1, 16):
            frame_number = 25 + 74 * (page - 1) + 3 * step
            values = coverage(frame(frame_number), page, colors)
            observations.append(
                [
                    page + 1,
                    kind,
                    option,
                    step / 16,
                    bytes(round(v * 255) for v in values).hex(),
                ]
            )
    return {
        "source": "Microsoft PowerPoint 16.115 for macOS, 30 fps H.264 export; 1600 ms transition and 767 ms hold",
        "movieSha256": hashlib.sha256(movie.read_bytes()).hexdigest(),
        "packagePayloadSha256": payloads(source),
        "frames": 912,
        "calibration": "wipe/r",
        "width": 96,
        "height": 54,
        "pixelIndices": SAMPLES,
        "samples": observations,
    }


def summarize(items):
    return {
        "values": len(items),
        "meanAbsoluteError": round(statistics.mean(items), 6),
        "p95AbsoluteError": round(sorted(items)[(len(items) * 95 - 1) // 100], 6),
        "maxAbsoluteError": round(max(items), 6),
    }


def verify(reference, baseline, candidate, out):
    out.mkdir(parents=True)
    original = out / "方向与分割.pptx"
    motion["generate"](original, CASES)
    assert payloads(original) == reference["packagePayloadSha256"]
    source = out / "sampling.pptx"
    motion["hold_for_sampling"](original, source, len(CASES))
    data = {}
    for label, cli in [("baseline", baseline), ("candidate", candidate)]:
        rows = reference["samples"]
        frames = motion["sample_cli"](cli, source, rows, out / label)
        errors = {}
        held = []
        for (slide, kind, option, t, encoded), frame in zip(rows, frames, strict=True):
            got = coverage(frame, slide - 1, SOURCE)
            diffs = [
                abs(actual - value / 255)
                for actual, value in zip(got, bytes.fromhex(encoded), strict=True)
            ]
            case = kind + "/" + option
            errors.setdefault(case, []).extend(diffs)
            if case != reference["calibration"]:
                held.extend(diffs)
        data[label] = {
            "directions": {k: summarize(v) for k, v in errors.items()},
            "heldOut": summarize(held),
        }
    assert (
        data["candidate"]["heldOut"]["meanAbsoluteError"]
        < data["baseline"]["heldOut"]["meanAbsoluteError"]
    )
    (out / "report.json").write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    for cmd, names in (
        ("generate", ["pptx"]),
        ("measure", ["movie", "pptx", "output"]),
        ("verify", ["reference", "baseline", "candidate", "output"]),
    ):
        part = sub.add_parser(cmd)
        for n in names:
            part.add_argument(n, type=Path)
        if cmd == "measure":
            part.add_argument("--rgb96x54", type=Path)
    args = parser.parse_args()
    if args.mode == "generate":
        motion["generate"](args.pptx, CASES)
    elif args.mode == "measure":
        args.output.write_text(
            json.dumps(measure(args.movie, args.pptx, args.rgb96x54), indent=2) + "\n"
        )
    else:
        verify(
            json.loads(args.reference.read_text()),
            args.baseline.resolve(),
            args.candidate.resolve(),
            args.output.resolve(),
        )
