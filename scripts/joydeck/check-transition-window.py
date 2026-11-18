"""Play an actual PPTX with device composition and with CPU rollback, observing the audience window."""

import argparse
import io
import json
import math
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import ROOT, DeckControl, app, digest, wait
from probe_native_ime import mac_keys, mac_target, run, system_events

P = "http://schemas.openxmlformats.org/presentationml/2006/main"
A = "http://schemas.openxmlformats.org/drawingml/2006/main"
P14 = "http://schemas.microsoft.com/office/powerpoint/2010/main"
COLORS = ["CC2211", "2352D0", "33C044"]
STANDARD = (("fade", ""), ("checker", "vert"))
TRAVEL = (("push", "l"), ("cover", "ru"))
MASKS = (("wipe", "ru"), ("split", "vertOut"))
ET.register_namespace("p", P)
ET.register_namespace("a", A)
ET.register_namespace("p14", P14)


def effect_xml(kind, option):
    if kind == "split":
        return "split", {"orient": option[:4], "dir": option[4:].lower()}
    if kind == "wipe" and len(option) == 2:
        return "strips", {"dir": option}
    tag = "pull" if kind == "uncover" else kind
    if option == "throughBlack":
        return tag, {"thruBlk": "1"}
    return tag, ({"dir": option} if option and option != "smoothly" else {})


def deck(path, effects=STANDARD, duration=4800, timed=False):
    with (
        ZipFile(ROOT / "tests/fixtures/joydeck/independent.pptx") as source,
        ZipFile(path, "x", ZIP_DEFLATED) as output,
    ):
        for name in source.namelist():
            data = source.read(name)
            page = re.fullmatch(r"ppt/slides/slide([123])\.xml", name)
            if page:
                i = int(page[1]) - 1
                root = ET.fromstring(data)
                cs = root.find("{*}cSld")
                shapes = cs.find("{*}spTree")
                for old in list(shapes)[2:]:
                    shapes.remove(old)
                for old in cs.findall("{*}bg"):
                    cs.remove(old)
                bg = ET.Element(f"{{{P}}}bg")
                cs.insert(0, bg)
                props = ET.SubElement(bg, f"{{{P}}}bgPr")
                fill = ET.SubElement(props, f"{{{A}}}solidFill")
                ET.SubElement(fill, f"{{{A}}}srgbClr", val=COLORS[i])
                ET.SubElement(props, f"{{{A}}}effectLst")
                if i:
                    tr = ET.SubElement(
                        root, f"{{{P}}}transition", {f"{{{P14}}}dur": str(duration)}
                    )
                    if timed and i == 1:
                        tr.set("advTm", "767")
                    kind, direction = effects[i - 1]
                    tag, attrs = effect_xml(kind, direction)
                    ET.SubElement(tr, f"{{{P}}}" + tag, attrs)
                data = ET.tostring(root, encoding="utf-8", xml_declaration=True)
            output.writestr(source.getinfo(name), data)


def window_id(pid, audience):
    if platform.system() == "Darwin":
        query = """ObjC.import("CoreGraphics"); let windows=ObjC.deepUnwrap(ObjC.castRefToObject(
            $.CGWindowListCopyWindowInfo(1,0))); JSON.stringify(windows.filter(
            w=>w.kCGWindowOwnerPID===PID && w.kCGWindowLayer===0).map(
            w=>[w.kCGWindowNumber,w.kCGWindowName||"",w.kCGWindowBounds.Width]));""".replace(
            "PID &&", f"{pid} &&"
        )
        windows = json.loads(run("osascript", "-l", "JavaScript", "-e", query))
        return next(
            (
                w[0]
                for w in windows
                if w[2] >= 300 and (w[1].endswith("· 放映")) == audience
            ),
            None,
        )
    ids = run("xdotool", "search", "--onlyvisible", "--pid", str(pid)).splitlines()
    return next(
        (
            w
            for w in ids
            if (run("xdotool", "getwindowname", w).strip().endswith("· 放映"))
            == audience
        ),
        None,
    )


def key(process, window, name):
    if platform.system() == "Darwin":
        target = mac_target(process.pid)
        system_events(
            f"if not frontmost of {target} then set frontmost of {target} to true"
        )
        mac_keys(
            process.pid,
            [{"f5": "96", "right": "124", "pause": "35", "escape": "53"}[name]],
        )
    else:
        run("xdotool", "windowactivate", "--sync", str(window))
        assert run("xdotool", "getactivewindow").strip() == str(window)
        run(
            "xdotool",
            "key",
            "--clearmodifiers",
            {"f5": "F5", "right": "Right", "pause": "p", "escape": "Escape"}[name],
        )


GRID = (256, 144)


def image_grid(image):
    if platform.system() == "Darwin":
        from PIL import Image, ImageCms

        with Image.open(image) as source:
            converted = source.convert("RGB")
            profile = source.info.get("icc_profile")
            if profile:
                converted = ImageCms.profileToProfile(
                    converted,
                    ImageCms.getOpenProfile(io.BytesIO(profile)),
                    ImageCms.createProfile("sRGB"),
                    outputMode="RGB",
                )
            data = converted.resize(GRID, Image.Resampling.NEAREST).tobytes()
    else:
        tool = shutil.which("magick") or shutil.which("convert")
        data = subprocess.check_output(
            [tool, str(image), "-sample", "256x144!", "-depth", "8", "rgb:-"]
        )
    assert len(data) == GRID[0] * GRID[1] * 3
    return list(zip(*[iter(data)] * 3))


def capture(window, image):
    if platform.system() == "Darwin":
        run("screencapture", "-x", "-o", "-l", str(window), str(image))
    else:
        run("import", "-silent", "-window", str(window), str(image))
    return image_grid(image)


def samples(grid, bounds):
    l, t, r, b = bounds
    return [
        grid[(t + round((b - t) * y / 49)) * GRID[0] + l + round((r - l) * x / 89)]
        for y in range(2, 48)
        for x in range(3, 87)
    ]


def average(pixels):
    return [round(statistics.mean(c), 2) for c in zip(*pixels)]


def reference_frame(binary, doc, directory, slide, seconds):
    image = directory / f"slide-{slide}-CPU.png"
    with app(
        binary,
        ["control", "--root", str(doc.parent)],
        directory,
        {},
        f"reference-{slide}.log",
    ) as proc:
        client = DeckControl(proc)
        opened = client.request("document.open", params={"path": str(doc)})
        guard = {"target": opened["documentId"], "expectedRevision": 0}
        client.request("show.start", params={"page": slide - 1}, **guard)
        client.request("show.action", params={"action": "next"}, **guard)
        status = client.request("show.tick", params={"seconds": seconds}, **guard)
        assert status["page"] == slide
        client.request("show.frame", params={"path": str(image), "scale": 0.6}, **guard)
    return samples(image_grid(image), (0, 0, GRID[0] - 1, GRID[1] - 1))


def close_color(a, b, tolerance):
    return max(abs(x - y) for x, y in zip(a, b)) <= tolerance


def stats(values):
    values = sorted(values)
    assert values
    return {
        "frames": len(values),
        "medianMs": round(statistics.median(values) / 1000, 2),
        "p95Ms": round(values[math.ceil(0.95 * len(values)) - 1] / 1000, 2),
    }


def play(binary, cli, doc, directory, mode, effects, software_fallback=False):
    directory.mkdir()
    settings = {
        "JOYDECK_GPU_TRANSITIONS": "0" if mode == "cpu" else "1",
        "JOYDECK_TRACE_TRANSITIONS": "1",
        "XDG_CONFIG_HOME": str(directory / "config"),
    }
    log = directory / "desktop.log"

    def contents():
        return log.read_text(errors="replace")

    records = {}
    with app(binary, [str(doc)], directory, settings) as process:
        wait(
            lambda: "opened=true pages=3" in contents(),
            "three-slide deck opened",
            process,
        )
        viewer = wait(lambda: window_id(process.pid, False), "editor window", process)
        key(process, viewer, "f5")
        audience = wait(
            lambda: window_id(process.pid, True), "audience window", process
        )
        wait(lambda: "show_first_frame" in contents(), "settled first slide", process)

        def first_bounds():
            grid = capture(audience, directory / "first.png")
            w, h = GRID
            red = [10 < p[0] and p[0] > p[2] + 45 and p[0] > p[1] + 45 for p in grid]
            rows = [y for y in range(h) if sum(red[y * w : (y + 1) * w]) > w * 0.5]
            cols = [x for x in range(w) if sum(red[x::w]) > h * 0.5]
            if len(rows) > h * 0.5 and len(cols) > w * 0.5:
                bounds = (cols[0], rows[0], cols[-1], rows[-1])
                return bounds, average(samples(grid, bounds))

        bounds, previous = wait(first_bounds, "first red slide on screen", process)
        for slide, (kind, _) in enumerate(effects, 2):
            key(process, audience, "right")
            pattern = re.compile(
                rf"show_transition_frame slide={slide} effect={kind} progress=([.\d]+) prepare_us=(\d+) pixels=(\d+)x(\d+)"
            )
            pause_from = (
                0.22
                if kind == "cover"
                else 0.30
                if kind in ("wipe", "split")
                else 0.38
                if kind == "push"
                else 0.48
            )
            wait(
                lambda: any(
                    pause_from < float(m[1]) < 0.88
                    for m in pattern.finditer(contents())
                ),
                f"visible {kind} progress",
                process,
            )
            key(process, audience, "pause")

            def paused_state():
                states = [
                    json.loads(line.split("show_state ", 1)[1])
                    for line in contents().splitlines()
                    if "R7 show_state " in line
                ]
                return next(
                    (q for q in reversed(states) if q["paused"] and q["page"] == slide),
                    None,
                )

            paused = wait(paused_state, "accepted pause", process)
            progress = paused["slide_seconds"] / 4.8
            wait(
                lambda: any(
                    abs(float(m[1]) - progress) < 1e-6
                    for m in pattern.finditer(contents())
                ),
                "prepared the frame at the paused clock",
                process,
            )
            reference = reference_frame(
                cli, doc, directory, slide, paused["slide_seconds"]
            )

            def observed_pause():
                pixels = samples(
                    capture(audience, directory / f"{kind}-paused.png"), bounds
                )
                middle = average(pixels)
                return (
                    (middle, pixels)
                    if close_color(middle, average(reference), 7)
                    else None
                )

            middle, pixels = wait(
                observed_pause, "paused pixels reached the audience", process
            )
            expected = average(reference)
            assert close_color(middle, expected, 7), (kind, middle, expected, paused)
            # At the same paused timestamp compare actual locations, allowing one
            # sampled neighbour for physical window scaling and colour quantization.
            mismatch = 0
            for i, actual in enumerate(pixels):
                row, col = divmod(i, 84)
                neighbours = (
                    reference[y * 84 + x]
                    for y in range(max(0, row - 1), min(46, row + 2))
                    for x in range(max(0, col - 1), min(84, col + 2))
                )
                if not any(close_color(actual, ref, 5) for ref in neighbours):
                    mismatch += 1
            assert mismatch <= len(pixels) * 0.02, (
                kind,
                mismatch,
                len(pixels),
                middle,
                expected,
            )
            assert not close_color(previous, middle, 5), (kind, previous, middle)
            key(process, audience, "pause")
            target = bytes.fromhex(COLORS[slide - 1])
            time.sleep(max(0.0, 4.8 - paused["slide_seconds"]))

            def settled():
                pixels = samples(
                    capture(audience, directory / f"{kind}-end.png"), bounds
                )
                return (
                    average(pixels)
                    if all(close_color(pixel, target, 3) for pixel in pixels)
                    else None
                )

            current = wait(settled, "settled slide on the display", process)
            assert not close_color(current, middle, 5) and not close_color(
                current, previous, 30
            )
            if kind == "fade":
                ratio = (middle[0] - previous[0]) / (current[0] - previous[0])
                assert 0.02 < ratio < 0.98
                assert close_color(
                    middle, [a + (b - a) * ratio for a, b in zip(previous, current)], 9
                )
            else:
                old = sum(close_color(p, previous, 5) for p in pixels)
                new = sum(close_color(p, current, 5) for p in pixels)
                for count, color in [(old, previous), (new, current)]:
                    reference_count = sum(close_color(p, color, 5) for p in reference)
                    assert abs(count - reference_count) <= len(pixels) * 0.08, (
                        count,
                        reference_count,
                    )
            times = list(pattern.finditer(contents()))
            records[kind] = {
                "referenceMean": expected,
                "outlyingSamples": mismatch,
                "samples": len(pixels),
                "pausedSeconds": paused["slide_seconds"],
                "firstColor": previous,
                "pausedColor": middle,
                "settledColor": current,
                "screenshots": {
                    tag: digest(directory / f"{kind}-{tag}.png")
                    for tag in ["paused", "end"]
                },
                "pixels": [int(n) for n in times[0].groups()[2:]],
                "preparation": stats(
                    [int(m[2]) for m in times if 0.01 < float(m[1]) < 0.99]
                ),
            }
            previous = current
        key(process, audience, "escape")
        wait(lambda: "show_closed" in contents(), "closed audience", process)
    text = contents()
    if mode == "cpu":
        assert (
            "backend=cpu reason=configuration" in text
            and "show_transition uploaded" not in text
        )
        backend = "CPU"
    elif software_fallback:
        match = re.search(r"backend=cpu reason=software adapter ([^\n]+)", text)
        assert match and "show_transition uploaded" not in text
        backend = f"CPU fallback / {match[1]}"
    else:
        backend = re.search(r'show_transition backend=(\w+) adapter="([^"]+)"', text)
        assert backend and "backend=cpu" not in text, text[-1200:]
        backend = f"{backend[1]} / {backend[2]}"
        assert {
            int(n) for n in re.findall(r"show_transition uploaded slide=(\d+)", text)
        } == {2, 3}
    return {"backend": backend, "slides": records}


def play_automatic(binary, directory):
    directory.mkdir()
    document = directory / "自动翻页.pptx"
    deck(document, TRAVEL, duration=1600, timed=True)
    with app(
        binary,
        [str(document)],
        directory,
        {
            "JOYDECK_TRACE_TRANSITIONS": "1",
            "XDG_CONFIG_HOME": str(directory / "config"),
        },
    ) as proc:
        log = directory / "desktop.log"
        editor = wait(lambda: window_id(proc.pid, False), "editor window", proc)
        wait(
            lambda: "opened=true pages=3" in log.read_text(errors="replace"),
            "timed presentation opened",
            proc,
        )
        key(proc, editor, "f5")
        audience = wait(
            lambda: window_id(proc.pid, True), "timed audience window", proc
        )
        wait(
            lambda: "show_first_frame" in log.read_text(errors="replace"),
            "first timed slide",
            proc,
        )

        def first_visible():
            bitmap = capture(audience, directory / "automatic-first.png")
            centre = bitmap[(GRID[1] // 2) * GRID[0] + GRID[0] // 2]
            return close_color(centre, tuple(bytes.fromhex(COLORS[0])), 4)

        wait(first_visible, "first timed slide actually presented", proc)
        if platform.system() == "Darwin":
            # AppKit can expose the fullscreen surface before its Space is active.
            time.sleep(1.0)
            audience = wait(
                lambda: window_id(proc.pid, True), "active fullscreen Space", proc
            )
            wait(first_visible, "settled fullscreen surface", proc)
        start = time.monotonic()
        key(proc, audience, "right")

        def next_slide():
            trace = log.read_text(errors="replace")
            if "show_transition_frame slide=3 effect=cover" in trace:
                return trace

        trace = wait(next_slide, "automatic advance to slide three", proc)
        elapsed = time.monotonic() - start
        before, _ = trace.split("show_transition_frame slide=3 effect=cover", 1)
        progress = [
            float(p)
            for p in re.findall(
                r"show_transition_frame slide=2 effect=push progress=([.\d]+)", before
            )
        ]
        assert progress and max(progress) > 0.85 and elapsed >= 2.30, (
            progress,
            elapsed,
        )
        key(proc, audience, "escape")
        wait(
            lambda: "show_closed" in log.read_text(errors="replace"),
            "closed timed audience",
            proc,
        )
        return {
            "durationMs": 1600,
            "holdMs": 767,
            "lastObservedPushProgress": max(progress),
            "observedFromKeyDispatchSeconds": round(elapsed, 3),
        }


def exchange(doc, directory, baseline, candidate, effects):
    directory.mkdir()
    versions = {"baseline": baseline, "candidate": candidate}
    readers = {}
    for writer, exe in versions.items():
        saved = directory / f"{writer}.pptx"
        subprocess.run(
            [exe, "exchange", doc, "--mode", "regenerate", "--output", saved],
            capture_output=True,
            check=True,
        )
        with ZipFile(saved) as z:
            for page, (kind, direction) in enumerate(effects, 2):
                slide = ET.fromstring(z.read(f"ppt/slides/slide{page}.xml"))
                transition = slide.find(".//{*}transition")
                tag, attrs = effect_xml(kind, direction)
                assert (
                    transition is not None and transition.get(f"{{{P14}}}dur") == "4800"
                )
                child = transition.find("{*}" + tag)
                assert child is not None and all(
                    child.get(k) == v for k, v in attrs.items()
                )
        for reader, tool in versions.items():
            work = directory / f"{writer}-{reader}"
            work.mkdir()
            for page in (2, 3):
                reference_frame(tool, saved, work, page, 2.4)
            readers[f"{writer}-{reader}"] = [
                digest(work / f"slide-{page}-CPU.png") for page in (2, 3)
            ]
    for reader in versions:
        assert readers[f"baseline-{reader}"] == readers[f"candidate-{reader}"]
    if effects == STANDARD:
        assert len({tuple(v) for v in readers.values()}) == 1
    return {
        "cliSha256": {k: digest(v) for k, v in versions.items()},
        "halfTimeFramesByWriterAndReader": readers,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--reference-cli", type=Path)
    parser.add_argument("--exchange-only", action="store_true")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--masks", action="store_true", help="exercise diagonal wipe and centered split"
    )
    mode.add_argument(
        "--travel", action="store_true", help="exercise push and diagonal cover"
    )
    parser.add_argument(
        "--expect-software-fallback",
        action="store_true",
        help="verify automatic CPU fallback on a desktop with only a software adapter",
    )
    args = parser.parse_args()
    effects = MASKS if args.masks else TRAVEL if args.travel else STANDARD
    out = args.output.resolve()
    cli = args.binary.with_name("joydeck-cli").resolve()
    baseline = (args.reference_cli or cli).resolve()
    doc = out / "转场 示例.pptx"
    if args.exchange_only:
        result = exchange(doc, out / "exchange", baseline, cli, effects)
        (out / "exchange.json").write_text(json.dumps(result, indent=2) + "\n")
        return
    out.mkdir(parents=True)
    deck(doc, effects)
    original = digest(doc)
    results = {}
    for mode in ("auto", "cpu"):
        results[mode] = play(
            args.binary.resolve(),
            cli if args.travel or args.masks else baseline,
            doc,
            out / mode,
            mode,
            effects,
            args.expect_software_fallback,
        )
    assert digest(doc) == original
    assert all(
        close_color(
            results["auto"]["slides"][k]["settledColor"],
            results["cpu"]["slides"][k]["settledColor"],
            1,
        )
        for k, _ in effects
    )
    (out / "report.json").write_text(
        json.dumps(
            {
                "sourceSha256": original,
                "binarySha256": digest(args.binary),
                "modes": results,
                **(
                    {
                        "automaticWindow": play_automatic(
                            args.binary.resolve(), out / "automatic"
                        )
                    }
                    if args.travel
                    else {}
                ),
                "exchange": exchange(doc, out / "exchange", baseline, cli, effects),
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
