"""Drag a rotated DrawingML callout leader in a native window, then check undo and the saved XML."""

import argparse
import hashlib
import itertools
import json
import math
from pathlib import Path
import platform
import runpy
import shutil
import struct
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import DeckControl, app, digest, wait
from probe_native_ime import mac_target, system_events

flow = runpy.run_path(str(Path(__file__).with_name("check-flowcharts.py")))
A = flow["A"]
KIND = "accentBorderCallout3"
GUIDES = [17000, 12000, 6000, -24000, 82000, -18000, 115000, 76000]
SIZE = (2650000, 1420000)
EXTRA = "{urn:joyoffice:preservation-example}"


def fixture(path):
    seed = path.with_name("generated.pptx")
    flow["board"](seed, [KIND], SIZE, False, adjustments={KIND: GUIDES})
    with ZipFile(seed) as before, ZipFile(path, "x", ZIP_DEFLATED) as after:
        for part in before.namelist():
            raw = before.read(part)
            if part == "ppt/slides/slide1.xml":
                tree = ET.fromstring(raw)
                shape = tree.find(".//{*}spTree/{*}sp/{*}spPr")
                frame = shape.find("{*}xfrm")
                frame.set("rot", str(19 * 60000))
                frame.set("flipH", "1")
                frame.find("{*}off").attrib.update(x="6000000", y="2800000")
                guides = shape.find("{*}prstGeom/{*}avLst")
                guides.set(EXTRA + "mode", "retained")
                for i, guide in enumerate(guides):
                    guide.set(EXTRA + "slot", str(i + 1))
                ET.SubElement(guides, f"{{{A}}}gd", name="otherGuide", fmla="*/ w 2 3")
                ET.SubElement(guides, EXTRA + "extra", value="kept").text = "附加数据"
                raw = ET.tostring(tree, encoding="utf-8")
            after.writestr(part, raw)


def inspect_xml(path):
    with ZipFile(path) as package:
        tree = ET.fromstring(package.read("ppt/slides/slide1.xml"))
    shape = tree.find(".//{*}spTree/{*}sp/{*}spPr")
    geom = shape.find("{*}prstGeom")
    assert geom.get("prst") == KIND
    guides = geom.find("{*}avLst")
    assert guides.get(EXTRA + "mode") == "retained"
    unknown = guides.find(EXTRA + "extra")
    assert unknown.text == "附加数据" and unknown.get("value") == "kept"
    known = {}
    for guide in guides.findall(f"{{{A}}}gd"):
        name = guide.get("name")
        if name == "otherGuide":
            assert guide.get("fmla") == "*/ w 2 3"
        else:
            i = int(name.removeprefix("adj"))
            assert guide.get(EXTRA + "slot") == str(i)
            known[i] = int(guide.get("fmla").split()[1])
    assert len(known) == 8 and any(g.get("name") == "otherGuide" for g in guides)
    return [known[i] for i in range(1, 9)], shape.find("{*}xfrm").attrib


def raw_rgb(png):
    data = png.read_bytes()
    w, h = struct.unpack(">2I", data[16:24])
    try:
        from PIL import Image
    except ImportError:
        pixels = subprocess.check_output(
            [
                shutil.which("magick") or "convert",
                str(png),
                "-alpha",
                "remove",
                "-depth",
                "8",
                "rgb:-",
            ]
        )
    else:
        with Image.open(png) as im:
            pixels = im.convert("RGB").tobytes()
    assert len(pixels) == 3 * w * h
    return w, h, pixels


class Window:
    def __init__(self, pid, output):
        self.mac = platform.system() == "Darwin"
        if self.mac:
            self.pointer = output / "native-drag"
            subprocess.run(
                [
                    "swiftc",
                    str(Path(__file__).with_name("native-drag.swift")),
                    "-o",
                    self.pointer,
                ],
                check=True,
            )
            system_events(f"set frontmost of {mac_target(pid)} to true")
            js = (
                'ObjC.import("CoreGraphics"); JSON.stringify(ObjC.deepUnwrap(ObjC.castRefToObject('
                f"$.CGWindowListCopyWindowInfo(1,0))).filter(x=>x.kCGWindowOwnerPID==={pid}&&x.kCGWindowLayer===0));"
            )
            windows = json.loads(
                subprocess.check_output(["osascript", "-l", "JavaScript", "-e", js])
            )
            win = max(windows, key=lambda q: q["kCGWindowBounds"]["Width"])
            self.id = str(win["kCGWindowNumber"])
            bounds = win["kCGWindowBounds"]
            self.width = bounds["Width"]
            self.origin = (bounds["X"], bounds["Y"])
        else:
            self.id = subprocess.check_output(
                ["xdotool", "search", "--onlyvisible", "--pid", str(pid)], text=True
            ).splitlines()[-1]
            subprocess.run(["xdotool", "windowactivate", "--sync", self.id], check=True)
            geo = subprocess.check_output(
                ["xdotool", "getwindowgeometry", "--shell", self.id], text=True
            )
            self.width = int(
                dict(row.split("=", 1) for row in geo.splitlines())["WIDTH"]
            )

    def capture(self, file):
        cmd = (
            ["screencapture", "-x", "-o", "-l", self.id, str(file)]
            if self.mac
            else ["import", "-window", self.id, str(file)]
        )
        subprocess.run(cmd, check=True, timeout=12)
        w, h, pixels = raw_rgb(file)
        self.scale = w / self.width
        # Amber geometry controls have a dark border. Rendering and screen colour
        # profiles can shift their channels, so compare the added regions against
        # an otherwise identical screenshot with the selection cleared.
        return {
            (i % w, i // w)
            for i, (r, g, b) in enumerate(struct.iter_unpack("3B", pixels))
            if r >= 202 and 140 <= g <= 229 and b < 136 and r - g >= 12 and g - b > 70
        }

    def drag(self, start, end):
        if self.mac:
            coordinates = [
                str(round(origin + offset))
                for point in (start, end)
                for origin, offset in zip(self.origin, point)
            ]
            subprocess.run([self.pointer, *coordinates], check=True)
        else:

            def move(p):
                return ["mousemove", "--window", self.id, *[str(round(x)) for x in p]]

            subprocess.run(
                [
                    "xdotool",
                    *move(start),
                    "mousedown",
                    "1",
                    "sleep",
                    ".12",
                    *move(tuple((a + b) / 2 for a, b in zip(start, end))),
                    "sleep",
                    ".12",
                    *move(end),
                    "sleep",
                    ".12",
                    "mouseup",
                    "1",
                ],
                check=True,
            )


def centers(mask, scale):
    points = set(mask)
    found = []
    while points:
        queue = [points.pop()]
        for x, y in queue:
            for neighbor in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if neighbor in points:
                    points.remove(neighbor)
                    queue.append(neighbor)
        xs, ys = zip(*queue)
        if (
            3 * scale <= max(xs) - min(xs) + 1 <= 12 * scale
            and 3 * scale <= max(ys) - min(ys) + 1 <= 12 * scale
        ):
            found.append(
                (
                    (min(xs) + max(xs) + 1) / (2 * scale),
                    (min(ys) + max(ys) + 1) / (2 * scale),
                )
            )
    return sorted(found)


def locate(guides, dots):
    theta = math.radians(19)

    def projected(i):
        x, y = (
            -guides[2 * i + 1] / 100000 * SIZE[0] / 12700,
            guides[2 * i] / 100000 * SIZE[1] / 12700,
        )
        return complex(x, y) * complex(math.cos(theta), math.sin(theta))

    known = [projected(i) for i in range(4)]
    assert len(dots) == 4, dots
    best = None
    for order in itertools.permutations(complex(x, y) for x, y in dots):
        a = sum(known) / 4
        b = sum(order) / 4
        zoom = sum(
            ((p - a).conjugate() * (q - b)).real for p, q in zip(known, order)
        ) / sum(abs(p - a) ** 2 for p in known)
        if zoom <= 0:
            continue
        error = max(abs((p - a) * zoom + b - q) for p, q in zip(known, order))
        if best is None or error < best[0]:
            best = error, zoom, list(order)
    assert best is not None and best[0] < 1.8, (best, dots)
    return best[1], best[2]


def run(binary, oldcli, newcli, out):
    out.mkdir(parents=True, exist_ok=False)
    src, saved = out / "旋转折线标注.pptx", out / "折点修改.pptx"
    fixture(src)
    assert inspect_xml(src)[0] == GUIDES
    original = digest(src)
    config = {
        "JOYDECK_CONFIG_DIR": str(out / "profile"),
        "JOYDECK_RECOVERY_DIR": str(out / "recovery"),
    }

    def launch(path):
        return [str(path), "--window-size=1100x740", "--control-stdio=" + str(out)]

    def read(c):
        return c.inspect()["slides"][0]["objects"][0]

    with app(binary, launch(src), out, config) as process:
        c = DeckControl(process)
        first = wait(lambda: read(c), "imported callout", process)
        window = Window(process.pid, out)
        c.edit({"command": {"command": "select", "page": 1, "shapes": []}})
        time.sleep(0.5)
        base = window.capture(out / "unselected.png")
        c.edit({"command": {"command": "select", "page": 1, "shapes": [first["id"]]}})

        def dots(label):
            time.sleep(0.4)
            return centers(window.capture(out / (label + ".png")) - base, window.scale)

        initial = dots("before")
        zoom, positions = locate(GUIDES, initial)
        # Move the third vertex which is deliberately outside the body. The
        # reflected, rotated shape makes a swapped axis or a slide-space edit observable.
        vertex = 2
        start = positions[vertex]
        delta = complex(32, -19)
        end = start + delta
        window.drag((start.real, start.imag), (end.real, end.imag))
        # Observe the frame after the OS releases the mouse before polling the
        # automation channel. A drag in progress deliberately rejects commands.
        changed = dots("after")
        wait(
            lambda: c.request("document.status")["dirty"],
            "native drag commits",
            process,
        )

        def near(actual, expected):
            assert len(actual) == 4
            assert (
                max(min(abs(complex(*a) - v) for a in actual) for v in expected) <= 2.5
            ), (actual, expected)

        final = list(positions)
        final[vertex] = end
        near(changed, final)
        c.edit({"command": {"command": "undo"}})
        near(dots("undo"), positions)
        c.edit({"command": {"command": "redo"}})
        near(dots("redo"), final)
        c.request("document.save", params={"path": str(saved)}, **c.guard())
        assert not c.request("document.status")["dirty"]
    numbers, mapping = inspect_xml(saved)
    assert mapping == inspect_xml(src)[1]
    changed_guides = [i for i, (a, b) in enumerate(zip(GUIDES, numbers)) if a != b]
    assert changed_guides == [2 * vertex, 2 * vertex + 1], numbers
    fitted, _ = locate(numbers, changed)
    assert abs(fitted - zoom) < 0.03
    with app(binary, launch(saved), out, config, "reopened.log") as process:
        c = DeckControl(process)
        shape = wait(lambda: read(c), "saved callout reopens", process)
        assert shape["transform"] == first["transform"]
    assert digest(src) == original
    pixels = {}
    for cli, label in ((oldcli, "old"), (newcli, "new")):
        for path, tag in ((src, "original"), (saved, "edited")):
            _, rgb = flow["support"]["render"](cli, path, out / (label + "-" + tag))
            pixels[label + "-" + tag] = hashlib.sha256(rgb).hexdigest()
    assert (
        pixels["old-edited"] == pixels["new-edited"]
        and pixels["new-original"] != pixels["new-edited"]
    ), pixels
    result = {
        "desktopSha256": digest(binary),
        "sourceSha256": original,
        "sourceUnchanged": True,
        "savedSha256": digest(saved),
        "selectedVertices": initial,
        "afterDrag": changed,
        "dragInWindowPixels": [delta.real, delta.imag],
        "zoom": zoom,
        "vertexIndex": vertex,
        "originalGuides": GUIDES,
        "savedGuides": numbers,
        "rotationDegrees": 19,
        "flipHorizontal": True,
        "unknownXmlRetained": True,
        "undoRedo": True,
        "freshProcessReopened": True,
        "renderedRgb": pixels,
    }
    (out / "report.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("desktop", "baseline", "candidate", "output"):
        parser.add_argument(key, type=Path)
    args = parser.parse_args()
    run(
        *[
            p.resolve()
            for p in (args.desktop, args.baseline, args.candidate, args.output)
        ]
    )
