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
PATTERNS = (("shape", "circle"), ("clock", "wedge"))
PANELS = (("blinds", "horz"), ("ripple", "lu"))
PRISMS = (("cube", "l"), ("orbit", "u"))
AIRBORNE = (("wind", "r"), ("prestige", ""))
PAPER = {"fragments": (("fracture", ""), ("crush", "")),
         "pages": (("peelOff", "r"), ("pageCurlDouble", "l")),
         "flight": (("airplane", "r"), ("origami", "l"))}
FABRIC = (("fallOver", "l"), ("curtains", ""))
PRESET_OPTIONS = {
    "fallOver": ("l", "r"), "drape": ("l", "r"), "curtains": ("",), "wind": ("l", "r"),
    "prestige": ("",), "fracture": ("",), "crush": ("",), "peelOff": ("l", "r"),
    "pageCurlDouble": ("l", "r", "singleL", "singleR"), "airplane": ("l", "r"), "origami": ("l", "r"),
}
ET.register_namespace("p", P)
ET.register_namespace("a", A)
ET.register_namespace("p14", P14)


def effect_xml(kind, option):
    tag, attrs, ns = kind, {}, P
    if kind in PRESET_OPTIONS:
        inverse = option == "l" if kind in ("wind", "origami", "airplane") else option in ("r", "singleR")
        preset = "pageCurlSingle" if kind == "pageCurlDouble" and option.startswith("single") else kind
        tag, ns = "prstTrans", "http://schemas.microsoft.com/office/powerpoint/2012/main"
        attrs = {"prst": preset, "invX": str(int(inverse))}
    elif kind == "shape":
        if option in ("in", "out"):
            tag, attrs = "zoom", {"dir": option}
        else:
            tag = option
    elif kind in ("cube", "box", "rotate", "orbit"):
        tag, ns = "prism", P14
        attrs = {"dir": option, "isContent": str(int(kind in ("rotate", "orbit"))),
                 "isInverted": str(int(kind in ("box", "orbit")))}
    elif kind == "ripple":
        ns, attrs = P14, {"dir": option or "center"}
    elif kind == "clock":
        if option == "wedge":
            tag = "wedge"
        else:
            tag, attrs = "wheel", {"spokes": "1"}
            if option == "counterClockwise":
                tag, ns = "wheelReverse", P14
    elif kind == "split":
        attrs = {"orient": option[:4], "dir": option[4:].lower()}
    elif kind == "wipe" and len(option) == 2:
        tag, attrs = "strips", {"dir": option}
    else:
        tag = "pull" if kind == "uncover" else kind
        if option == "throughBlack":
            attrs = {"thruBlk": "1"}
        elif option and option != "smoothly":
            attrs = {"dir": option}
    return f"{{{ns}}}" + tag, attrs


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
                if 0 < i <= len(effects):
                    tr = ET.SubElement(
                        root, f"{{{P}}}transition", {f"{{{P14}}}dur": str(duration)}
                    )
                    if timed and i == 1:
                        tr.set("advTm", "767")
                    kind, direction = effects[i - 1]
                    tag, attrs = effect_xml(kind, direction)
                    ET.SubElement(tr, tag, attrs)
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


def image_grid(image, crop=None):
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
            if crop:
                converted = converted.crop(crop)
            data = converted.resize(GRID, Image.Resampling.NEAREST).tobytes()
    else:
        tool = shutil.which("magick") or shutil.which("convert")
        options = []
        if crop:
            l, t, r, b = crop
            options = ["-crop", f"{r-l}x{b-t}+{l}+{t}", "+repage"]
        data = subprocess.check_output([tool, str(image), *options, "-sample", "256x144!", "-depth", "8", "rgb:-"])
    assert len(data) == GRID[0] * GRID[1] * 3
    return list(zip(*[iter(data)] * 3))


def capture(window, image, crop=None):
    if platform.system() == "Darwin":
        run("screencapture", "-x", "-o", "-l", str(window), str(image))
    else:
        run("import", "-silent", "-window", str(window), str(image))
    return image_grid(image, crop)


def slide_crop(image):
    """Locate the solid first slide in physical pixels before resampling fine transition edges."""
    if platform.system() == "Darwin":
        from PIL import Image
        with Image.open(image) as source:
            rgb = source.convert("RGB")
            w, h = rgb.size
            row = [rgb.getpixel((x, h // 2)) for x in range(w)]
            col = [rgb.getpixel((w // 2, y)) for y in range(h)]
    else:
        tool = shutil.which("magick") or shutil.which("convert")
        w, h = map(int, subprocess.check_output([tool, str(image), "-format", "%w %h", "info:"]).split())
        def scanline(rect):
            data = subprocess.check_output([tool, str(image), "-crop", rect, "+repage", "-depth", "8", "rgb:-"])
            return zip(*[iter(data)] * 3)
        row = scanline(f"{w}x1+0+{h // 2}")
        col = scanline(f"1x{h}+{w // 2}+0")

    def red_pixels(line):
        return [i for i, (r, g, b) in enumerate(line) if r > b + 45 and r > g + 45]
    xs, ys = red_pixels(row), red_pixels(col)
    if len(xs) > w * 0.5 and len(ys) > h * 0.5:
        return xs[0], ys[0], xs[-1] + 1, ys[-1] + 1
    return None


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
        {"XDG_CONFIG_HOME": str(directory / "reference-config"),
         "XDG_STATE_HOME": str(directory / "reference-state")},
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


def play(binary, cli, doc, directory, mode, effects, duration_ms, software_fallback=False):
    directory.mkdir()
    settings = {
        "JOYDECK_GPU_TRANSITIONS": "0" if mode == "cpu" else "1",
        "JOYDECK_TRACE_TRANSITIONS": "1",
        "XDG_CONFIG_HOME": str(directory / "config"),
        "XDG_STATE_HOME": str(directory / "state"),
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
            image = directory / "first.png"
            capture(audience, image)
            crop = slide_crop(image)
            if crop:
                return crop, average(samples(image_grid(image, crop), bounds))

        bounds = (0, 0, GRID[0] - 1, GRID[1] - 1)
        crop, previous = wait(first_bounds, "first red slide on screen", process)
        for slide, (kind, _) in enumerate(effects, 2):
            key(process, audience, "right")
            pattern = re.compile(
                rf"show_transition_frame slide={slide} effect={kind} progress=([.\d]+) prepare_us=(\d+) pixels=(\d+)x(\d+)"
            )
            pause_from = (
                (0.12 if mode == "cpu" else 0.37)
                if kind == "curtains"
                else (0.28 if mode == "cpu" else 0.55)
                if kind == "fallOver"
                else 0.22
                if kind == "cover"
                else 0.30
                if kind in ("wipe", "split", "blinds")
                else 0.38
                if kind == "push"
                else 0.48
            )
            def pause_ready():
                frames = list(pattern.finditer(contents()))
                if kind not in PRESET_OPTIONS or kind in ("fallOver", "drape", "curtains"):
                    return any(pause_from < float(frame[1]) < 0.88 for frame in frames)
                if not frames:
                    return False
                progress = float(frames[-1][1])
                # The key can land after the frame already in preparation and
                # the one that receives the event. Measure each actual display.
                lead = 2 * int(frames[-1][2]) / (duration_ms * 1000)
                if kind == "prestige":
                    # The canopy is stationary during its first third.
                    return progress > 0.36 and progress + lead >= 0.61
                return progress + lead >= 0.49

            wait(pause_ready, f"visible {kind} progress", process)
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
            progress = paused["slide_seconds"] / (duration_ms / 1000)
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

            expected = average(reference)
            latest = {}

            def observed_pause():
                pixels = samples(capture(audience, directory / f"{kind}-paused.png", crop), bounds)
                middle = average(pixels)
                # Both the compositor's color and its spatial pattern must reach
                # the requested timestamp before accepting an asynchronous frame.
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
                latest.update(mean=middle, differing=mismatch)
                if close_color(middle, expected, 7) and mismatch <= len(pixels) * 0.02:
                    return middle, pixels, mismatch
                return None

            try:
                middle, pixels, mismatch = wait(observed_pause, "paused pixels reached the audience", process)
            except (RuntimeError, TimeoutError) as error:
                raise RuntimeError(f"{kind} at {paused['slide_seconds']:.4f}s, expected {expected}, observed {latest}") from error
            assert not close_color(previous, middle, 5), (kind, previous, middle)
            key(process, audience, "pause")
            target = bytes.fromhex(COLORS[slide - 1])
            time.sleep(max(0.0, duration_ms / 1000 - paused["slide_seconds"]))

            def settled():
                pixels = samples(
                    capture(audience, directory / f"{kind}-end.png", crop), bounds
                )
                return (
                    average(pixels)
                    # A slow CPU composition can approach the final color while
                    # it is still showing an intermediate frame. Keep this bound
                    # as strict as the cross-backend endpoint comparison below.
                    if all(close_color(pixel, target, 1) for pixel in pixels)
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
        } == set(range(2, 2 + len(effects)))
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
            "XDG_STATE_HOME": str(directory / "state"),
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


def exchange(doc, directory, baseline, candidate, effects, duration_ms):
    directory.mkdir()
    versions = {"baseline": baseline, "candidate": candidate}
    readers = {}
    for writer, exe in versions.items():
        saved = directory / f"{writer}.pptx"
        subprocess.run(
            [exe, "exchange", doc, "--mode", "regenerate", "--output", saved],
            env={**os.environ, "XDG_CONFIG_HOME": str(directory / "config"),
                 "XDG_STATE_HOME": str(directory / "state")},
            capture_output=True,
            check=True,
        )
        with ZipFile(saved) as z:
            for page, (kind, direction) in enumerate(effects, 2):
                slide = ET.fromstring(z.read(f"ppt/slides/slide{page}.xml"))
                transition = slide.find(".//{*}transition")
                tag, attrs = effect_xml(kind, direction)
                assert (
                    transition is not None and transition.get(f"{{{P14}}}dur") == str(duration_ms)
                )
                child = transition.find(tag)
                assert child is not None and all(
                    (child.get(k, "0") in ("true", "1")) == (v == "1")
                    if k in ("isContent", "isInverted", "invX") else child.get(k) == v
                    for k, v in attrs.items()
                )
        for reader, tool in versions.items():
            work = directory / f"{writer}-{reader}"
            work.mkdir()
            for page in range(2, 2 + len(effects)):
                reference_frame(tool, saved, work, page, duration_ms / 2000)
            readers[f"{writer}-{reader}"] = [
                digest(work / f"slide-{page}-CPU.png") for page in range(2, 2 + len(effects))
            ]
    for reader in versions:
        assert readers[f"baseline-{reader}"] == readers[f"candidate-{reader}"]
    for index, (kind, _) in enumerate(effects):
        if kind == "fade":
            assert len({frames[index] for frames in readers.values()}) == 1
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
    mode.add_argument("--paper", choices=PAPER, help="exercise two paper effects on the actual audience window")
    mode.add_argument("--airborne", action="store_true", help="exercise a gust and a central lift of the outgoing slide")
    mode.add_argument("--fabric", action="store_true", help="exercise a falling sheet and two parting curtains")
    mode.add_argument("--prisms", action="store_true", help="exercise an outside turn and content-only inside turn")
    mode.add_argument("--ripples", choices=["center", "lu", "ru", "ld", "rd"], help="exercise the selected ripple in the audience window")
    mode.add_argument("--panels", action="store_true", help="exercise Venetian blinds and a corner ripple")
    mode.add_argument(
        "--patterns",
        action="store_true",
        help="exercise a circular reveal and a wedge in the audience window",
    )
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
    effects = (
        PAPER[args.paper]
        if args.paper
        else AIRBORNE
        if args.airborne
        else FABRIC
        if args.fabric
        else PRISMS
        if args.prisms
        else (("ripple", args.ripples),)
        if args.ripples
        else PANELS
        if args.panels
        else PATTERNS
        if args.patterns
        else MASKS
        if args.masks
        else TRAVEL
        if args.travel
        else STANDARD
    )
    out = args.output.resolve()
    cli = args.binary.with_name("joydeck-cli").resolve()
    baseline = (args.reference_cli or cli).resolve()
    doc = out / "转场 示例.pptx"
    # Allow the full-window debug CPU compositor to present an intermediate
    # frame before a fast-moving cloth has completely cleared the viewport.
    duration_ms = 9600 if (args.airborne or args.paper) else 4800
    if args.exchange_only:
        result = exchange(doc, out / "exchange", baseline, cli, effects, duration_ms)
        (out / "exchange.json").write_text(json.dumps(result, indent=2) + "\n")
        return
    out.mkdir(parents=True)
    deck(doc, effects, duration=duration_ms)
    original = digest(doc)
    results = {}
    for mode in ("auto", "cpu"):
        results[mode] = play(
            args.binary.resolve(),
            cli,
            doc,
            out / mode,
            mode,
            effects,
            duration_ms,
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
                "exchange": exchange(doc, out / "exchange", baseline, cli, effects, duration_ms),
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
