"""Check shape edits, theme roles and style inheritance against the frozen JoyDeck binaries."""

import argparse
import json
from pathlib import Path
import runpy
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from PIL import Image, ImageChops


COMMON = runpy.run_path(str(Path(__file__).with_name("check-stage2-transfer.py")))
FIXTURE, SLIDE, LAYOUT, MASTER, THEME, NS = (
    COMMON[key] for key in ("FIXTURE", "SLIDE", "LAYOUT", "MASTER", "THEME", "NS")
)
add, find, tag, edit, run, model, sha, frame = (
    COMMON[key] for key in ("add", "find", "tag", "edit", "run", "model", "sha", "transform")
)
SLIDES = (SLIDE.replace("slide2", "slide1"), SLIDE)
QA = "urn:joyoffice:stage2-shapes-theme"
ET.register_namespace("qa", QA)
COLORS = {
    "dk1": "16283C", "lt1": "F3F5E7", "dk2": "334655", "lt2": "D0DEEC",
    "accent1": "1658A4", "accent2": "E27632", "accent3": "459B58", "accent4": "764ACB",
    "accent5": "3AA5C2", "accent6": "C74778", "hlink": "047376", "folHlink": "94275F",
}
MAP = {"bg1": "lt1", "tx1": "dk1", "bg2": "lt2", "tx2": "dk2"} | {
    key: key for key in COLORS if key.startswith("accent") or "link" in key.lower()
}
MASTER_MAP = MAP | {"bg1": "lt2", "tx1": "dk2", "accent1": "accent3"}
LAYOUT_MAP = MAP | {"bg1": "dk1", "tx1": "lt1", "accent1": "accent2", "accent2": "accent1",
                    "hlink": "folHlink", "folHlink": "hlink"}
SLIDE_MAP = MAP | {"accent1": "accent4", "hlink": "accent5"}
NEW_ACCENT = "26A14D"
# Expected from the actual PowerPoint 16 reference export.
CORRECTED_TRANSFORMS = {"Inverse and gray": "EFEFEF", "RGB modulation": "355177"}

# (name, color element, color value, ordered transforms)
PROBES = [
    ("Accent", "schemeClr", "accent1", ()),
    ("Other accent", "schemeClr", "accent2", ()),
    ("Hyperlink", "schemeClr", "hlink", ()),
    ("Followed link", "schemeClr", "folHlink", ()),
    ("Text role", "schemeClr", "tx1", ()),
    ("Physical dark", "schemeClr", "dk1", ()),
    ("Tint then shade", "srgbClr", "126AC2", (("tint", 25000), ("shade", 60000))),
    ("Shade then tint", "srgbClr", "126AC2", (("shade", 60000), ("tint", 25000))),
    ("Luminance", "srgbClr", "2468AC", (("lumMod", 80000), ("lumOff", 14000))),
    ("Saturation", "srgbClr", "2468AC", (("satMod", 77000), ("satOff", 5000))),
    ("Hue", "srgbClr", "2468AC", (("hueMod", 80000), ("hueOff", -1500000))),
    ("Complement", "srgbClr", "2468AC", (("comp", None),)),
    ("Inverse and gray", "srgbClr", "2468AC", (("inv", None), ("gray", None))),
    ("RGB modulation", "srgbClr", "2468AC", (("redMod", 200000), ("greenMod", 60000), ("blueMod", 45000))),
    ("Ordered alpha", "srgbClr", "2468AC", (("alpha", 80000), ("alphaMod", 50000), ("alphaOff", 10000))),
    ("Alpha with clamp", "srgbClr", "336699", (("alphaOff", 50000), ("alphaMod", 100000))),
    ("Linear tint", "srgbClr", "000000", (("tint", 50000),)),
]


def replace(parent, name, element):
    old = parent.find(name, NS)
    index = list(parent).index(old) if old is not None else len(parent)
    if old is not None:
        parent.remove(old)
    parent.insert(index, element)


def color(parent, kind, value, mods=()):
    container = add(parent, f"a:{kind}", val=value)
    for op, val in mods:
        add(container, f"a:{op}", **({"val": str(val)} if val is not None else {}))
    return container


def solid(parent, value, kind="schemeClr", mods=()):
    color(add(parent, "a:solidFill"), kind, value, mods)


def pose(parent, kind="rect"):
    add(add(parent, "a:prstGeom", prst=kind), "a:avLst")


def shape(tree, ident, name, bounds=None, ph=None):
    sp = add(tree, "p:sp")
    nv = add(sp, "p:nvSpPr")
    add(nv, "p:cNvPr", id=str(ident), name=name)
    add(nv, "p:cNvSpPr")
    props = add(nv, "p:nvPr")
    if ph is not None:
        add(props, "p:ph", type=ph[0], idx=str(ph[1]))
    props = add(sp, "p:spPr")
    if bounds is not None:
        frame(props, *bounds)
    return sp, props


def group(tree, ident, bounds):
    node = add(tree, "p:grpSp")
    nv = add(node, "p:nvGrpSpPr")
    add(nv, "p:cNvPr", id=str(ident), name=f"Group {ident}")
    add(nv, "p:cNvGrpSpPr")
    add(nv, "p:nvPr")
    frame(add(node, "p:grpSpPr"), *bounds, group=True)
    return node


def theme_override(root, mapping=None):
    override = ET.Element(tag("p:clrMapOvr"))
    if mapping is None:
        add(override, "a:masterClrMapping")
    else:
        add(override, "a:overrideClrMapping", **mapping)
    replace(root, "p:clrMapOvr", override)


def tree(root):
    sp_tree = find(root, "p:cSld/p:spTree")
    for child in list(sp_tree)[2:]:
        sp_tree.remove(child)
    return sp_tree


def make_theme(path, *, mapped=True):
    with ZipFile(FIXTURE) as template, ZipFile(path, "x") as out:
        roots = {name: ET.fromstring(template.read(name)) for name in (*SLIDES, LAYOUT, MASTER, THEME)}
        master = roots[MASTER]
        layout = roots[LAYOUT]
        scheme = find(roots[THEME], "a:themeElements/a:clrScheme")
        for slot, value in COLORS.items():
            entry = find(scheme, f"a:{slot}")
            entry.clear()
            color(entry, "srgbClr", value)
        fmt = find(roots[THEME], "a:themeElements/a:fmtScheme")
        main = find(fmt, "a:fillStyleLst")[0]
        main.clear()
        main.tag = tag("a:solidFill")
        color(main, "schemeClr", "phClr", (("shade", 65000),))
        outline = find(fmt, "a:lnStyleLst")[0]
        for child in list(outline):
            if child.tag == tag("a:solidFill"):
                outline.remove(child)
        fill = ET.Element(tag("a:solidFill"))
        color(fill, "schemeClr", "phClr")
        outline.insert(0, fill)
        background = find(fmt, "a:bgFillStyleLst")[0]
        background.clear()
        background.tag = tag("a:solidFill")
        color(background, "schemeClr", "phClr")
        master_data = find(master, "p:cSld")
        for child in master_data.findall("p:bg", NS):
            master_data.remove(child)
        bg = ET.Element(tag("p:bg"))
        color(add(bg, "p:bgRef", idx="1001"), "schemeClr", "bg1")
        master_data.insert(0, bg)
        find(master, "p:clrMap").attrib.update(MASTER_MAP if mapped else MAP)

        _, props = shape(tree(master), 74, "Master body", (400, 255, 160, 92), ("body", 9))
        pose(props)
        solid(add(props, "a:ln", w=str(8 * 12700)), "accent1")
        layout_tree = tree(layout)
        # This first placeholder has the same type but a different index.
        shape(layout_tree, 78, "Other body", (590, 255, 160, 92), ("body", 4))
        body, _ = shape(layout_tree, 79, "Layout body", ph=("body", 9))
        style = add(body, "p:style")
        color(add(style, "a:lnRef", idx="1"), "schemeClr", "hlink")
        color(add(style, "a:fillRef", idx="1"), "schemeClr", "accent1", (("tint", 78000),))
        color(add(style, "a:effectRef", idx="0"), "schemeClr", "phClr")
        color(add(style, "a:fontRef", idx="minor"), "schemeClr", "tx1")
        theme_override(layout, LAYOUT_MAP if mapped else None)

        for i, name in enumerate(SLIDES, 1):
            root = roots[name]
            root.set("showMasterSp", "0")
            shapes = tree(root)
            base = i * 1000
            for j, (label, kind, val, mods) in enumerate(PROBES):
                x, y = 24 + j % 6 * 151, 30 + j // 6 * 64
                _, prop = shape(shapes, base + j + 1, label, (x, y, 102, 50))
                pose(prop)
                solid(prop, val, kind, mods if mapped or label not in CORRECTED_TRANSFORMS else ())
                add(add(prop, "a:ln"), "a:noFill")
            _, prop = shape(shapes, base + 40, "Edit geometry", (211, 250, 110, 94))
            geom = add(prop, "a:prstGeom", prst="roundRect")
            add(add(geom, "a:avLst"), "a:gd", name="adj", fmla="val 23000")
            solid(prop, "627589", "srgbClr")
            add(add(prop, "a:ln"), "a:noFill")
            inherited, prop = shape(shapes, base + 41, "Inherited body", ph=("body", 9))
            # Width comes from slide properties. Ink comes from the layout's phClr style.
            add(prop, "a:ln", w=str(10 * 12700))
            text = add(inherited, "p:txBody")
            add(text, "a:bodyPr")
            add(text, "a:lstStyle")
            add(add(text, "a:p"), "a:endParaRPr", lang="en-US")
            first_group = group(shapes, base + 50, (39, 250, 120, 103))
            inner = group(first_group, base + 51, (0, 0, 120, 103))
            custom, prop = shape(inner, base + 52, "Custom with extension", (2, 3, 103, 85))
            geom = add(prop, "a:custGeom")
            add(geom, "a:avLst")
            add(geom, "a:gdLst")
            add(geom, "a:ahLst")
            add(geom, "a:cxnLst")
            add(geom, "a:rect", l="l", t="t", r="r", b="b")
            path_geom = add(add(geom, "a:pathLst"), "a:path", w="100", h="100")
            for verb, x, y in [("moveTo", "10", "10"), ("lnTo", "95", "10"), ("lnTo", "45", "95")]:
                add(add(path_geom, f"a:{verb}"), "a:pt", x=x, y=y)
            add(path_geom, "a:close")
            solid(prop, "accent6")
            add(add(prop, "a:ln"), "a:noFill")
            ext = add(add(custom, "p:extLst"), "p:ext", uri=QA)
            ET.SubElement(ext, f"{{{QA}}}sentinel").text = "preserve shape extension"
            theme_override(root, SLIDE_MAP if i == 2 and mapped else None)
        for item in template.infolist():
            data = template.read(item)
            if item.filename in roots:
                data = ET.tostring(roots[item.filename], encoding="utf-8", xml_declaration=True)
            out.writestr(item, data)


def render(binary, source, folder, page):
    run(binary, "render", source, "--output", folder, "--page", str(page), "--scale", "2")
    with Image.open(folder / f"slide-{page:04}.png") as image:
        return image.convert("RGB")


def rgb(value):
    return tuple(bytes.fromhex(value))


def pixel(image, x, y):
    return image.getpixel((x * 2, y * 2))


def shade_tint(swatch, *, tint=None, shade=None):
    def channel(byte, factor, white):
        c = byte / 255
        linear = c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
        changed = linear * factor + white
        srgb = changed * 12.92 if changed <= 0.0031308 else 1.055 * changed ** (1 / 2.4) - 0.055
        return int(srgb * 255 + 0.5)

    for factor, white in [(tint, 1 - tint if tint is not None else 0), (shade, 0)]:
        if factor is not None:
            swatch = tuple(channel(byte, factor, white) for byte in swatch)
    return swatch


def check_render(frozen, candidate, page, changed_accent, report):
    mapping = (LAYOUT_MAP, SLIDE_MAP)[page - 1]
    palette = COLORS | ({"accent2": NEW_ACCENT} if changed_accent else {})
    expected = {
        "Background": (900, 470, rgb(palette[mapping["bg1"]])),
        "Inherited fill": (470, 300, shade_tint(rgb(palette[mapping["accent1"]]), tint=0.78, shade=0.65)),
        "Inherited outline": (401, 300, rgb(palette[mapping["hlink"]])),
    }
    for j, (name, kind, value, mods) in enumerate(PROBES):
        x, y = 60 + j % 6 * 151, 50 + j // 6 * 64
        if kind == "schemeClr":
            swatch = rgb(palette[mapping.get(value, value)])
        elif name == "Tint then shade":
            swatch = shade_tint(rgb(value), tint=0.25, shade=0.60)
        elif name == "Shade then tint":
            swatch = shade_tint(shade_tint(rgb(value), shade=0.60), tint=0.25)
        elif name == "Linear tint":
            swatch = (188, 188, 188)
        elif name in CORRECTED_TRANSFORMS:
            swatch = rgb(CORRECTED_TRANSFORMS[name])
        elif name == "Ordered alpha":
            alpha = (int(int(0.8 * 255 + 0.5) * 0.5 + 0.5) + int(0.1 * 255 + 0.5)) / 255
            swatch = tuple(round(fg * alpha + bg * (1 - alpha)) for fg, bg in
                           zip(rgb(value), rgb(palette[mapping["bg1"]])))
        else:
            swatch = pixel(frozen, x, y)
        expected[name] = (x, y, swatch)
    for name, (x, y, expected_rgb) in expected.items():
        actual = pixel(candidate, x, y)
        tolerance = 1 if name == "Ordered alpha" else 0
        assert all(abs(a - b) <= tolerance for a, b in zip(actual, expected_rgb)), (
            page, name, actual, expected_rgb)
        report["pixelProbes"] += 1
        if pixel(frozen, x, y) != actual:
            kind = "changedTransformProbes" if name in CORRECTED_TRANSFORMS else "changedMappingProbes"
            report[kind].add(name)


def op(kind, **data):
    return {"command": "content", "action": {"kind": kind, **data}}


def verify_xml(path, start, accent, shape_preset):
    with ZipFile(path) as book, ZipFile(start) as original:
        slides = [ET.fromstring(book.read(name)) for name in SLIDES]
        shape_xml = find(slides[0], "p:cSld/p:spTree")
        target = next(sp for sp in shape_xml.findall("p:sp", NS)
                      if find(sp, "p:nvSpPr/p:cNvPr").attrib["name"] == "Edit geometry")
        assert find(target, "p:spPr/a:prstGeom").attrib["prst"] == shape_preset
        sentinel = slides[0].find(f".//{{{QA}}}sentinel")
        assert sentinel is not None and sentinel.text == "preserve shape extension"
        assert find(slides[1], "p:clrMapOvr/a:overrideClrMapping").attrib == SLIDE_MAP
        layout = ET.fromstring(book.read(LAYOUT))
        assert find(layout, "p:clrMapOvr/a:overrideClrMapping").attrib == LAYOUT_MAP
        master = ET.fromstring(book.read(MASTER))
        assert find(master, "p:clrMap").attrib == MASTER_MAP
        theme = ET.fromstring(book.read(THEME))
        assert find(theme, "a:themeElements/a:clrScheme/a:accent2/a:srgbClr").attrib["val"] == accent
        for part in book.namelist():
            if part.startswith(("ppt/notes", "ppt/media")) and part in original.namelist():
                assert book.read(part) == original.read(part), part


def pptx_order(path):
    with ZipFile(path) as book:
        root = ET.fromstring(book.read("ppt/presentation.xml"))
        rels = ET.fromstring(book.read("ppt/_rels/presentation.xml.rels"))
        targets = {rel.attrib["Id"]: rel.attrib["Target"] for rel in rels}
        return [(node.attrib["id"], targets[node.attrib[tag("r:id")]])
                for node in find(root, "p:sldIdLst")]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    args.folder.mkdir(parents=True, exist_ok=True)
    source = args.folder / "Shapes and Theme QA.pptx"
    neutral = args.folder / "Shapes and Theme Neutral.pptx"
    for path, mapped in [(source, True), (neutral, False)]:
        if not path.is_file():
            make_theme(path, mapped=mapped)
    if not args.frozen and not args.candidate:
        return
    if not args.frozen or not args.candidate:
        parser.error("both CLI binaries are required")
    bins = {"frozen": args.frozen.resolve(), "candidate": args.candidate.resolve()}
    input_sha = sha(source)
    original = model(bins["frozen"], source)
    assert original == model(bins["candidate"], source)
    report = {"fixtureSha256": input_sha, "binaries": {key: sha(binary) for key, binary in bins.items()},
              "neutralRasters": 0, "pixelProbes": 0, "changedMappingProbes": set(),
              "changedTransformProbes": set(), "readbacks": 0,
              "historyReadbacks": 0, "packageChecks": 0, "rejections": 0}

    for page in (1, 2):
        f, c = [render(binary, neutral, args.folder / f"{name}-neutral-{page}", page)
                for name, binary in bins.items()]
        assert ImageChops.difference(f, c).getbbox() is None, ("neutral", page)
        report["neutralRasters"] += 1
        f, c = [render(binary, source, args.folder / f"{name}-mapped-{page}", page)
                for name, binary in bins.items()]
        check_render(f, c, page, False, report)

    slide = original["slides"][0]
    target = next(shape for shape in slide["shapes"] if shape["name"] == "Edit geometry")
    group_before = next(shape for shape in slide["shapes"] if shape["name"] == "Group 1050")
    alternate = next(layout for layout in original["masters"][0]["layouts"] if layout["id"] != slide["layout"])
    selection = {"command": "select", "page": 1, "shapes": [target["id"]]}
    theme = [op("theme-accent", slot=2, rgb=int(NEW_ACCENT, 16))]
    before = theme + [selection, op("shape-preset", name="ellipse")]
    layout = before + [op("layout", id=alternate["id"])]
    pages = [s["id"] for s in original["slides"]]
    order = pptx_order(source)
    first_objects = [s["shapes"][0]["id"] for s in original["slides"]]
    reorder = layout + [{"command": "workspace", "action": {
        "kind": "reorder-slides", "slides": [pages[1]], "before": pages[0],
    }}]
    undo, redo = {"command": "undo"}, {"command": "redo"}
    commands = {"theme": theme, "shape": before, "layout": layout, "reorder": reorder,
                "undo-layout": reorder + [undo, undo], "undo-shape": reorder + [undo, undo, undo],
                "redo-reorder": reorder + [undo, undo, redo, redo]}
    undo_of = {"undo-layout": "shape", "undo-shape": "theme", "redo-reorder": "reorder"}
    snapshots = {}
    for writer, binary in bins.items():
        for key, script in commands.items():
            state = undo_of.get(key, key)
            for ext in ("joydeck", "pptx"):
                saved = edit(binary, source, args.folder, f"{writer}-{key}", script, fmt=ext)
                if ext == "pptx":
                    assert pptx_order(saved) == ([order[1], order[0], *order[2:]] if state == "reorder" else order)
                if ext == "pptx" and state in ("theme", "shape"):
                    verify_xml(saved, source, NEW_ACCENT, "roundRect" if state == "theme" else "ellipse")
                    report["packageChecks"] += 1
                for reader, cli in bins.items():
                    data = model(cli, saved)
                    first = [s["shapes"][0]["id"] for s in data["slides"]]
                    assert first == ([first_objects[1], first_objects[0], *first_objects[2:]]
                                     if state == "reorder" else first_objects)
                    if ext == "joydeck":
                        ids = [s["id"] for s in data["slides"]]
                        assert ids == ([pages[1], pages[0], *pages[2:]] if state == "reorder" else pages)
                    edited = data["slides"][first.index(first_objects[0])]
                    expected_layout = alternate["id"] if state in ("layout", "reorder") else slide["layout"]
                    assert edited["layout"] == expected_layout, (state, edited["layout"], expected_layout)
                    edited_shape = next(s for s in edited["shapes"] if s["id"] == target["id"])
                    if state == "theme":
                        assert edited_shape == target
                    else:
                        assert edited_shape == target | {"geom": {"kind": "preset", "name": "ellipse"}}
                    kept = next(s for s in edited["shapes"] if s["id"] == group_before["id"])
                    assert kept == group_before
                    k = (ext, state)
                    if k in snapshots:
                        assert data == snapshots[k], (writer, reader, ext, key)
                        if key in undo_of:
                            report["historyReadbacks"] += 1
                    else:
                        snapshots[k] = data
                    report["readbacks"] += 1
        for i, script in enumerate(([op("theme-accent", slot=7, rgb=0)], [selection, op("shape-preset", name="bogus")])):
            assert edit(binary, source, args.folder, f"{writer}-reject-{i}", script, ok=False)
            report["rejections"] += 1

    for page in (1, 2):
        f, c = [render(binary, args.folder / f"{name}-theme.pptx",
                       args.folder / f"{name}-changed-accent-{page}", page) for name, binary in bins.items()]
        check_render(f, c, page, True, report)
    # Switching the slide to a layout without an override exposes the master mapping.
    inherited = render(bins["candidate"], args.folder / "candidate-layout.pptx",
                       args.folder / "candidate-master-mapping", 1)
    for role, x, y in [("accent1", 60, 50), ("tx1", 664, 50), ("bg1", 900, 470)]:
        assert pixel(inherited, x, y) == rgb(COLORS[MASTER_MAP[role]]), role
        report["pixelProbes"] += 1
    assert sha(source) == input_sha
    report["changedMappingProbes"] = sorted(report["changedMappingProbes"])
    report["changedTransformProbes"] = sorted(report["changedTransformProbes"])
    (args.folder / "shapes-theme-results.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
