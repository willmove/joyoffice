"""Replay stage 2 coordinate edits against the frozen and candidate JoyDeck binaries."""

import argparse
from dataclasses import dataclass, replace
import json
import math
from pathlib import Path
import runpy
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from PIL import Image, ImageChops


# Reuse the existing independent package and CLI helpers. Geometry below is computed from the
# DrawingML operations on points, independently of either binary's coordinate implementation.
COMMON = runpy.run_path(str(Path(__file__).with_name("check-stage2-transfer.py")))
FIXTURE, SLIDE, NS = (COMMON[key] for key in ("FIXTURE", "SLIDE", "NS"))
add, edit, find, model, run, sha, tag = (
    COMMON[key] for key in ("add", "edit", "find", "model", "run", "sha", "tag")
)
QA = "urn:joyoffice:coordinate-validation"
MC = "http://schemas.openxmlformats.org/markup-compatibility/2006"
ET.register_namespace("qa", QA)
ET.register_namespace("mc", MC)

PAGE = 2
EMU_PER_POINT = 12700
TOLERANCE = 0.001
COLORS = {105: "236F20", 106: "1639E5", 108: "CA2482", 110: "D96107"}
ATTACHMENTS = {
    105: ((103, 3), (104, 1)),
    106: ((104, 0), (109, 1)),
    108: ((103, 0), (109, 2)),
    110: ((103, 99), (104, 3)),
}
TYPES = {105: "straightConnector1", 106: "lineInv", 108: "bentConnector3", 110: "curvedConnector3"}
PARENTS = {101: None, 102: 101, 103: 102, 104: 102, 105: 102, 106: 102, 108: None, 109: None, 110: None}


@dataclass(frozen=True)
class Frame:
    x: float
    y: float
    w: float
    h: float
    rot: float = 0
    flipH: bool = False
    flipV: bool = False


FRAMES = {
    101: Frame(345, 205, 215, 155, 23, flipH=True),
    102: Frame(36, 11, 95, 47, 318, flipV=True),
    103: Frame(-6, 15, 28, 18, 14, flipH=True),
    104: Frame(69, 42, 18, 15, 303, flipV=True),
    109: Frame(695, 243, 48, 28, 13),
}
CHILDREN = {101: Frame(14, -8, 135, 90), 102: Frame(-16, 9, 108, 55)}
DETACHED = (648.0, 370.0)


def rotate(point, angle):
    x, y = point
    radians = math.radians(angle)
    return (math.cos(radians) * x - math.sin(radians) * y,
            math.sin(radians) * x + math.cos(radians) * y)


def place(frame, point):
    x, y = point
    x = frame.w - x if frame.flipH else x
    y = frame.h - y if frame.flipV else y
    dx, dy = rotate((x - frame.w / 2, y - frame.h / 2), frame.rot)
    return frame.x + frame.w / 2 + dx, frame.y + frame.h / 2 + dy


def through_groups(frames, parent, point):
    while parent is not None:
        frame, child = frames[parent], CHILDREN[parent]
        x, y = point
        point = place(frame, ((x - child.x) * frame.w / child.w, (y - child.y) * frame.h / child.h))
        parent = PARENTS[parent]
    return point


def site(frames, binding):
    shape_id, index = binding
    frame = frames[shape_id]
    sites = ((frame.w / 2, 0), (0, frame.h / 2), (frame.w / 2, frame.h), (frame.w, frame.h / 2))
    if index >= len(sites):
        return DETACHED
    return through_groups(frames, PARENTS[shape_id], place(frame, sites[index]))


def inverse_group(frame, child, point):
    dx, dy = rotate((point[0] - frame.x - frame.w / 2, point[1] - frame.y - frame.h / 2), -frame.rot)
    x = frame.w / 2 + (-dx if frame.flipH else dx)
    y = frame.h / 2 + (-dy if frame.flipV else dy)
    return child.x + x * child.w / frame.w, child.y + y * child.h / frame.h


def authored_connector(connector):
    points = [site(FRAMES, binding) for binding in ATTACHMENTS[connector]]
    if PARENTS[connector] is not None:
        for group in (101, 102):
            points = [inverse_group(FRAMES[group], CHILDREN[group], point) for point in points]
    (x0, y0), (x1, y1) = points
    angle = 17 if connector == 106 else 0
    dx, dy = rotate((x1 - x0, y1 - y0), -angle)
    rising = TYPES[connector] == "lineInv"
    return Frame((x0 + x1 - abs(dx)) / 2, (y0 + y1 - abs(dy)) / 2,
                 abs(dx), abs(dy), angle, dx < 0, (dy < 0) != rising)


def xml_frame(props, frame, child=None):
    xfrm = add(props, "a:xfrm", rot=str(round(frame.rot * 60000)),
               flipH=str(int(frame.flipH)), flipV=str(int(frame.flipV)))

    def units(value):
        return str(round(value * EMU_PER_POINT))

    add(xfrm, "a:off", x=units(frame.x), y=units(frame.y))
    add(xfrm, "a:ext", cx=units(frame.w), cy=units(frame.h))
    if child is not None:
        add(xfrm, "a:chOff", x=units(child.x), y=units(child.y))
        add(xfrm, "a:chExt", cx=units(child.w), cy=units(child.h))
    return xfrm


def drawing(parent, kind, shape_id, name):
    node = add(parent, f"p:{kind}")
    nv, cnv, props = {
        "grpSp": ("nvGrpSpPr", "cNvGrpSpPr", "grpSpPr"),
        "sp": ("nvSpPr", "cNvSpPr", "spPr"),
        "cxnSp": ("nvCxnSpPr", "cNvCxnSpPr", "spPr"),
    }[kind]
    nonvisual = add(node, f"p:{nv}")
    add(nonvisual, "p:cNvPr", id=str(shape_id), name=name)
    options = add(nonvisual, f"p:{cnv}")
    add(nonvisual, "p:nvPr")
    return node, options, add(node, f"p:{props}")


def fixture(destination):
    with ZipFile(FIXTURE) as original, ZipFile(destination, "x") as output:
        slide = ET.fromstring(original.read(SLIDE))
        slide.set("showMasterSp", "0")
        slide.set(f"{{{MC}}}Ignorable", "qa")
        tree = find(slide, "p:cSld/p:spTree")
        for child in list(tree):
            if child.tag not in {tag("p:nvGrpSpPr"), tag("p:grpSpPr")}:
                tree.remove(child)
        groups = {None: tree}
        for shape_id, name in ((101, "Coordinate group"), (102, "Inner coordinate group")):
            group, _, props = drawing(groups[PARENTS[shape_id]], "grpSp", shape_id, name)
            xml_frame(props, FRAMES[shape_id], CHILDREN[shape_id])
            groups[shape_id] = group
        for shape_id, color in ((103, "E9BB40"), (104, "40BCD1"), (109, "7661A6")):
            _, _, props = drawing(groups[PARENTS[shape_id]], "sp", shape_id, f"Target {shape_id}")
            xml_frame(props, FRAMES[shape_id])
            add(add(props, "a:prstGeom", prst="rect"), "a:avLst")
            add(add(props, "a:solidFill"), "a:srgbClr", val=color)
            add(add(props, "a:ln"), "a:noFill")
        for connector, (start, end) in ATTACHMENTS.items():
            node, options, props = drawing(groups[PARENTS[connector]], "cxnSp", connector, f"Connector {connector}")
            for name, (shape_id, index) in zip(("stCxn", "endCxn"), (start, end)):
                add(options, f"a:{name}", id=str(shape_id), idx=str(index))
            xfrm = xml_frame(props, authored_connector(connector))
            if connector == 106:
                xfrm.set(f"{{{QA}}}keep", "nested-transform")
                find(xfrm, "a:off").set(f"{{{QA}}}keep", "nested-offset")
                marker = add(add(node, "p:extLst"), "p:ext", uri="{7B66129D-E457-4F57-A392-43D8E10EA711}")
                ET.SubElement(marker, f"{{{QA}}}marker").text = "original nested connector"
            add(add(props, "a:prstGeom", prst=TYPES[connector]), "a:avLst")
            add(add(add(props, "a:ln", w="44450"), "a:solidFill"), "a:srgbClr", val=COLORS[connector])
        for member in original.infolist():
            data = ET.tostring(slide, encoding="utf-8", xml_declaration=True) if member.filename == SLIDE else original.read(member)
            output.writestr(member, data)


DIRECT = [
    {"command": "select", "page": PAGE, "shapes": [101]},
    {"command": "transform", "dx": 32, "dy": -18, "dw": 58, "dh": -24, "rotation": 48},
]
WHOLE = DIRECT + [
    {"command": "select", "page": PAGE, "shapes": [101, 109]},
    {"command": "workspace", "action": {"kind": "transform-selection", "dx": -14, "dy": 13, "scale_x": 1.16, "scale_y": 0.84, "rotation": -37}},
]
REPLAYS = {
    "direct": DIRECT,
    "scaled-rotated": WHOLE,
    "undo": WHOLE + [{"command": "undo"}],
    "redo": WHOLE + [{"command": "undo"}, {"command": "redo"}],
}


def expected_frames(step):
    frames = FRAMES.copy()
    if step is None:
        return frames
    frames[101] = replace(frames[101], x=377, y=187, w=273, h=131, rot=71)
    if step in {"scaled-rotated", "redo"}:
        points = [place(frames[shape_id], point) for shape_id in (101, 109)
                  for point in ((0, 0), (frames[shape_id].w, 0), (0, frames[shape_id].h), (frames[shape_id].w, frames[shape_id].h))]
        center = [(min(axis) + max(axis)) / 2 for axis in zip(*points)]
        for shape_id in (101, 109):
            f = frames[shape_id]
            dx, dy = rotate(((f.x + f.w / 2 - center[0]) * 1.16, (f.y + f.h / 2 - center[1]) * 0.84), -37)
            w, h = f.w * 1.16, f.h * 0.84
            frames[shape_id] = replace(f, x=center[0] + dx - 14 - w / 2, y=center[1] + dy + 13 - h / 2,
                                       w=w, h=h, rot=(f.rot - 37) % 360)
    return frames


def flatten(deck):
    frames, parents, children, endpoints = {}, {}, {}, {}

    def walk(shapes, parent):
        for shape in shapes:
            shape_id, kind = shape["id"], shape["kind"]
            frames[shape_id] = Frame(**shape["xfrm"])
            parents[shape_id] = parent
            if kind["type"] == "group":
                children[shape_id] = Frame(**kind["child"])
                walk(kind["children"], shape_id)
            if kind["type"] == "connector":
                endpoints[shape_id] = (tuple(kind["start"]), tuple(kind["end"]))
                assert shape["geom"]["name"] == TYPES[shape_id], shape

    walk(deck["slides"][PAGE - 1]["shapes"], None)
    assert parents == PARENTS, (parents, PARENTS)
    assert children == CHILDREN, (children, CHILDREN)
    assert endpoints == ATTACHMENTS, endpoints
    return frames


def connector_ends(frames, connector):
    frame = frames[connector]
    y = frame.h if TYPES[connector] == "lineInv" else 0
    return tuple(through_groups(frames, PARENTS[connector], place(frame, point))
                 for point in ((0, y), (frame.w, frame.h - y)))


def verify(deck, step, *, frozen_writer=False):
    frames = flatten(deck)
    expected = expected_frames(step)
    for shape_id, original in expected.items():
        found = frames[shape_id]
        assert (found.flipH, found.flipV) == (original.flipH, original.flipV), (shape_id, found, original)
        for name in ("x", "y", "w", "h", "rot"):
            assert abs(getattr(found, name) - getattr(original, name)) <= TOLERANCE, (step, shape_id, found, original)
    errors = {}
    for connector, attachments in ATTACHMENTS.items():
        ends = connector_ends(frames, connector)
        distances = [math.dist(actual, site(expected, binding)) for actual, binding in zip(ends, attachments)]
        errors[connector] = distances
        for index, error in enumerate(distances):
            stale = frozen_writer and step is not None and (connector, index) in {(106, 1), (108, 0), (110, 1)}
            stale |= frozen_writer and step in {"scaled-rotated", "redo"} and (connector, index) == (108, 1)
            assert error > 10 if stale else error <= TOLERANCE, (step, connector, index, error)
    return frames, errors


def inspect_preserved(path, source):
    with ZipFile(path) as output, ZipFile(source) as original:
        assert set(output.namelist()) == set(original.namelist())
        assert all(output.read(name) == original.read(name) for name in original.namelist() if name != SLIDE)
        tree = ET.fromstring(output.read(SLIDE))
        for node in tree.findall(".//p:cxnSp", NS):
            if find(node, "p:nvCxnSpPr/p:cNvPr").get("id") == "106":
                transform = find(node, "p:spPr/a:xfrm")
                assert transform.get(f"{{{QA}}}keep") == "nested-transform"
                assert find(transform, "a:off").get(f"{{{QA}}}keep") == "nested-offset"
                assert find(node, "p:extLst/p:ext").findtext(f"{{{QA}}}marker") == "original nested connector"
                return
    raise AssertionError("nested connector missing from preserved PPTX")


def raster(binary, source, folder):
    run(binary, "render", source, "--output", folder, "--page", str(PAGE), "--scale", "2")
    return Image.open(folder / f"slide-{PAGE:04}.png").convert("RGB")


def verify_raster(render, frames):
    for connector, color in COLORS.items():
        expected = tuple(bytes.fromhex(color))
        for x, y in connector_ends(frames, connector):
            px, py = round(x * 2), round(y * 2)
            samples = render.crop((px - 5, py - 5, px + 6, py + 6)).getcolors()
            assert samples and any(c == expected for _, c in samples), (connector, x, y, color)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--frozen", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    args = parser.parse_args()
    args.folder.mkdir(parents=True, exist_ok=True)
    path = args.folder / "Nested coordinates.pptx"
    fixture(path)
    binaries = {"frozen": args.frozen, "candidate": args.candidate}
    evidence = {"fixtureSha256": sha(path), "binaries": {name: sha(binary) for name, binary in binaries.items()},
                "endpointTolerancePt": TOLERANCE, "readbacks": 0, "preservedPackages": 0, "rasterChecks": 0,
                "exactUndoRedoComparisons": 0, "endpointErrorsPt": {}}
    original_rasters = []
    for name, binary in binaries.items():
        frames, _ = verify(model(binary, path), None)
        image = raster(binary, path, args.folder / f"{name}-input-raster")
        verify_raster(image, frames)
        original_rasters.append(image)
        evidence["rasterChecks"] += 1
    assert ImageChops.difference(*original_rasters).getbbox() is None
    evidence["originalRasterPixelIdentical"] = True

    prior_step = {"undo": "direct", "redo": "scaled-rotated"}
    for writer, binary in binaries.items():
        snapshots = {}
        for name, commands in REPLAYS.items():
            for ext in ("joydeck", "pptx"):
                output = edit(binary, path, args.folder, f"{writer}-{name}", commands, fmt=ext)
                if ext == "pptx":
                    inspect_preserved(output, path)
                    evidence["preservedPackages"] += 1
                for reader, reader_binary in binaries.items():
                    opened = model(reader_binary, output)
                    frames, errors = verify(opened, name, frozen_writer=writer == "frozen")
                    if name in prior_step:
                        assert opened == snapshots[ext, reader, prior_step[name]], (writer, reader, ext, name)
                        evidence["exactUndoRedoComparisons"] += 1
                    else:
                        snapshots[ext, reader, name] = opened
                    evidence["readbacks"] += 1
                    if reader == writer:
                        evidence["endpointErrorsPt"][f"{writer}-{name}.{ext}"] = errors
                        if writer == "candidate" and ext == "pptx":
                            image = raster(binary, output, args.folder / f"{writer}-{name}-raster")
                            verify_raster(image, frames)
                            evidence["rasterChecks"] += 1
    result = json.dumps(evidence, indent=2) + "\n"
    (args.folder / "coordinate-verification.json").write_text(result, encoding="utf-8")
    print(f"OK: {evidence['readbacks']} readbacks, {evidence['exactUndoRedoComparisons']} history comparisons, "
          f"{evidence['preservedPackages']} preserved packages, {evidence['rasterChecks']} endpoint rasters")


if __name__ == "__main__":
    main()
