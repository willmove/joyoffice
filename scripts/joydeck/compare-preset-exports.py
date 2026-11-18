"""Measure stable fill and outline pixels against PowerPoint PDF exports rasterized at 72 DPI."""

import argparse
import json
import runpy
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

suite = runpy.run_path(str(Path(__file__).with_name("check-standard-presets.py")))


def rgb(file):
    with Image.open(file) as image:
        assert image.size == (960, 540)
        return np.asarray(image.convert("RGB")).astype(np.int16)


def grow(mask):
    im = Image.fromarray(mask.astype("uint8") * 255)
    return np.asarray(im.filter(ImageFilter.MaxFilter(3))) > 0


def masks(image):
    # A 1px neighborhood covers the PDF and Kurbo anti-aliasing difference.
    return (image.min(axis=2) < 197, image.max(axis=2) < 99)


def solid_samples(image, origin, size):
    x, y = origin
    result = []
    for line in range(9):
        for col in range(9):
            px = int((x + (col + 0.5) * size[0] / 9) / 12700)
            py = int((y + (line + 0.5) * size[1] / 9) / 12700)
            sample = image[py - 1 : py + 2, px - 1 : px + 2]
            if np.all(sample == [30, 112, 206]):
                result.append("1")
            elif np.all(sample == 255):
                result.append("0")
            else:
                result.append(".")
    return "".join(result)


def run(office, rendered, out):
    names = suite["names"]()
    reports = {}
    hits = []
    for i in range(4):
        for state in suite["STATES"]:
            key = f"JoyOffice-presets-{i + 1}-{state}"
            dirs = {
                k: sorted((rendered / key / k).glob("*.png"))
                for k in ("baseline", "candidate")
            }
            for page, size in enumerate(suite["SIZES"].values(), 1):
                ref = rgb(office / f"{key}-{page}.png")
                refmasks = masks(ref)
                screens = {k: rgb(files[page - 1]) for k, files in dirs.items()}
                distances = {}
                for k, image in screens.items():
                    distances[k] = [
                        (a & ~grow(b)) | (b & ~grow(a))
                        for a, b in zip(refmasks, masks(image))
                    ]
                for ix, name in enumerate(names[i * 27 : i * 27 + 27]):
                    x, y = suite["flow"]["origin"](ix, size)
                    # Fixed tiles partition the slide, including callout tails and star tips.
                    left = (ix % 6) * 1990000 / 12700
                    top = (ix // 6) * 1320000 / 12700
                    region = (
                        slice(round(top), min(540, round(top + 1320000 / 12700))),
                        slice(round(left), min(960, round(left + 1990000 / 12700))),
                    )
                    if state == "default":
                        samples = solid_samples(ref, (x, y), size)
                        hits.append(f"{name} {size[0]} {size[1]} {samples}")
                    reports.setdefault(name, {})[f"{state}-{page}"] = {
                        k: [int(np.count_nonzero(mask[region])) for mask in m]
                        for k, m in distances.items()
                    }
    assert len(reports) == 108 and all(len(r) == 9 for r in reports.values())
    (out / "comparison.json").write_text(json.dumps(reports, indent=1) + "\n")
    (out / "standard-hits.txt").write_text("\n".join(hits) + "\n")
    for ver in ("baseline", "candidate"):
        for axis in (0, 1):
            values = [
                (q[ver][axis], n, state)
                for n, variants in reports.items()
                for state, q in variants.items()
            ]
            print(
                ver,
                ("silhouette", "stroke")[axis],
                sum(v for v, _, _ in values),
                sorted(values, reverse=True)[:12],
            )

    # At the reference resolution, even the thinnest star tips tolerate at
    # most ten isolated pixel differences per shape and mask, across renderers.
    for name, variants in reports.items():
        for setting, result in variants.items():
            assert max(result["candidate"]) <= 10, (name, setting, result)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("office", "rendered", "output"):
        parser.add_argument(name, type=Path)
    a = parser.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    run(a.office, a.rendered, a.output)
