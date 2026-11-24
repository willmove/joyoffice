"""Replay version-1 object transfers against two JoyDeck binaries and inspect both file formats."""

import argparse
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import posixpath
import subprocess
from xml.etree import ElementTree as ET
from zipfile import ZipFile


ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tests/fixtures/joydeck/independent.pptx"
SLIDE = "ppt/slides/slide2.xml"
SLIDE_RELS = "ppt/slides/_rels/slide2.xml.rels"
LAYOUT = "ppt/slideLayouts/slideLayout7.xml"
LAYOUT_RELS = "ppt/slideLayouts/_rels/slideLayout7.xml.rels"
MASTER = "ppt/slideMasters/slideMaster1.xml"
THEME = "ppt/theme/theme1.xml"
IMAGE = "ppt/media/image1.png"
BASE_REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
NS = {
    "a": "http://schemas.openxmlformats.org/drawingml/2006/main",
    "p": "http://schemas.openxmlformats.org/presentationml/2006/main",
    "r": BASE_REL,
    "rel": "http://schemas.openxmlformats.org/package/2006/relationships",
}
WEB = "https://example.org/joydeck-stage2"
GROUP = "Transfer group"
PLACEHOLDER = "Layout test"

for prefix, uri in NS.items():
    ET.register_namespace(prefix, uri)


def tag(name):
    prefix, name = name.split(":")
    return f"{{{NS[prefix]}}}{name}"


def add(parent, element, **attrs):
    return ET.SubElement(parent, tag(element), attrs)


def find(parent, name):
    child = parent.find(name, NS)
    assert child is not None, name
    return child


def rel(parent, label, kind, target, external=False):
    attrs = {"Id": label, "Type": f"{BASE_REL}/{kind}", "Target": target}
    if external:
        attrs["TargetMode"] = "External"
    add(parent, "rel:Relationship", **attrs)


def transform(parent, left, top, width, height, *, group=False):
    xfrm = add(parent, "a:xfrm")
    offset = dict(x=str(int(left * 12700)), y=str(int(top * 12700)))
    extent = dict(cx=str(int(width * 12700)), cy=str(int(height * 12700)))
    add(xfrm, "a:off", **offset)
    add(xfrm, "a:ext", **extent)
    if group:
        add(xfrm, "a:chOff", x="0", y="0")
        add(xfrm, "a:chExt", **extent)


def title_placeholder(tree, shape_id, text=None):
    node = add(tree, "p:sp")
    props = add(node, "p:nvSpPr")
    add(props, "p:cNvPr", id=str(shape_id), name=PLACEHOLDER)
    add(props, "p:cNvSpPr")
    add(add(props, "p:nvPr"), "p:ph", type="title", idx="91")
    sp = add(node, "p:spPr")
    tx = add(node, "p:txBody")
    add(tx, "a:bodyPr")
    add(tx, "a:lstStyle")
    p = add(tx, "a:p")
    if text is not None:
        add(add(p, "a:r"), "a:t").text = text
    return sp


def prepare(folder):
    folder.mkdir(parents=True, exist_ok=True)
    source = folder / "跨文稿 来源.pptx"
    destination = folder / "跨文稿 目标.pptx"
    if not destination.exists():
        destination.write_bytes(FIXTURE.read_bytes())
    assert destination.read_bytes() == FIXTURE.read_bytes(), destination
    if source.exists():
        return source, destination
    with ZipFile(FIXTURE) as archive:
        members = archive.infolist()
        files = {member.filename: archive.read(member) for member in members}
    parts = {name: ET.fromstring(files[name]) for name in (SLIDE, SLIDE_RELS, LAYOUT, LAYOUT_RELS, MASTER, THEME)}
    tree = find(parts[SLIDE], "p:cSld/p:spTree")
    normal_pic = find(tree, "p:pic")
    rel(parts[SLIDE_RELS], "rIdStage2Web", "hyperlink", WEB, external=True)
    rel(parts[SLIDE_RELS], "rIdStage2Slide", "slide", "slide3.xml")
    add(find(normal_pic, "p:nvPicPr/p:cNvPr"), "a:hlinkClick", **{tag("r:id"): "rIdStage2Web"})
    group = add(tree, "p:grpSp")
    non_visual = add(group, "p:nvGrpSpPr")
    name = add(non_visual, "p:cNvPr", id="100", name=GROUP)
    add(name, "a:hlinkClick", action="ppaction://hlinksldjump", **{tag("r:id"): "rIdStage2Slide"})
    add(non_visual, "p:cNvGrpSpPr")
    add(non_visual, "p:nvPr")
    transform(add(group, "p:grpSpPr"), 83, 327, 270, 110, group=True)

    pic = deepcopy(normal_pic)
    name = find(pic, "p:nvPicPr/p:cNvPr")
    name.set("id", "101")
    name.set("name", "Grouped image")
    picture_props = find(pic, "p:spPr")
    picture_props.remove(find(picture_props, "a:xfrm"))
    transform(picture_props, 0, 0, 112, 75)
    picture_props.insert(0, picture_props[-1])
    del picture_props[-1]
    group.append(pic)

    caption = deepcopy(find(tree, "p:sp"))
    name = find(caption, "p:nvSpPr/p:cNvPr")
    name.set("id", "102")
    name.set("name", "Grouped link")
    props = find(caption, "p:spPr")
    props.remove(find(props, "a:xfrm"))
    transform(props, 126, 0, 142, 50)
    props.insert(0, props[-1])
    del props[-1]
    run = find(caption, "p:txBody/a:p/a:r")
    props = ET.Element(tag("a:rPr"))
    run.insert(0, props)
    props.set("sz", "1800")
    add(props, "a:hlinkClick", **{tag("r:id"): "rIdStage2Web"})
    find(run, "a:t").text = "Group link"
    group.append(caption)

    line = add(group, "p:cxnSp")
    non_visual = add(line, "p:nvCxnSpPr")
    add(non_visual, "p:cNvPr", id="103", name="Grouped connector")
    connector = add(non_visual, "p:cNvCxnSpPr")
    add(connector, "a:stCxn", id="101", idx="1")
    add(connector, "a:endCxn", id="102", idx="2")
    add(non_visual, "p:nvPr")
    props = add(line, "p:spPr")
    transform(props, 111, 35, 25, 5)
    add(add(props, "a:prstGeom", prst="line"), "a:avLst")

    title_placeholder(tree, 104, "Layout stamp")
    layout_tree = find(parts[LAYOUT], "p:cSld/p:spTree")
    layout_shape = title_placeholder(layout_tree, 100, None)
    transform(layout_shape, 493, 348, 242, 73)
    blip = add(layout_shape, "a:blipFill")
    add(blip, "a:blip", **{tag("r:embed"): "rIdStage2LayoutImage"})
    add(add(blip, "a:stretch"), "a:fillRect")
    rel(parts[LAYOUT_RELS], "rIdStage2LayoutImage", "image", "../media/image1.png")
    title = find(parts[MASTER], "p:txStyles/p:titleStyle/a:lvl1pPr")
    run = find(title, "a:defRPr")
    for child in list(run):
        run.remove(child)
    add(add(run, "a:solidFill"), "a:schemeClr", val="accent2")
    add(run, "a:latin", typeface="+mj-lt")
    find(parts[THEME], "a:themeElements/a:fontScheme/a:majorFont/a:latin").set("typeface", "Arial")
    rgb = find(parts[THEME], "a:themeElements/a:clrScheme/a:accent2/a:srgbClr")
    rgb.set("val", "A15721")
    for name, node in parts.items():
        files[name] = ET.tostring(node, encoding="utf-8", xml_declaration=True)
    with ZipFile(source, "x") as out:
        for member in members:
            out.writestr(member, files[member.filename])
    return source, destination


def run(binary, *args, ok=True):
    result = subprocess.run([binary, *args], capture_output=True, text=True, encoding="utf-8", timeout=60)
    if ok != (result.returncode == 0):
        raise AssertionError((args[0], result.returncode, result.stderr))
    return result.stdout if ok else result.stderr.strip()


def model(cli, path):
    return json.loads(run(cli, "content", path))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def edit(cli, source, folder, name, commands, fmt="joydeck", ok=True):
    script = folder / f"{name}-{fmt}.json"
    script.write_text(json.dumps(commands), encoding="utf-8")
    dest = folder / f"{name}.{fmt}"
    outcome = run(cli, "edit", source, "--script", script, "--output", dest, ok=ok)
    if not ok:
        assert not dest.exists()
        return outcome
    return dest


def native_media(path):
    from io import BytesIO
    with ZipFile(path) as outer, ZipFile(BytesIO(outer.read("model.deckcraft"))) as native:
        return {n: native.read(n) for n in native.namelist() if n.startswith("media/")}


def layout_bindings(deck):
    layouts = {layout["id"]: (master["name"], layout["name"], layout["kind"])
               for master in deck["masters"] for layout in master["layouts"]}
    return [layouts[slide["layout"]] for slide in deck["slides"]]


def check_pptx(path, original, expected_names):
    with ZipFile(path) as output, ZipFile(original) as source:
        media = [n for n in output.namelist() if n.startswith("ppt/media/")]
        assert media == [IMAGE], media
        assert output.read(IMAGE) == source.read(IMAGE)
        kept = ("ppt/slideLayouts/", "ppt/slideMasters/", "ppt/theme/")
        assert all(source.read(n) == output.read(n) for n in source.namelist() if n.startswith(kept))
        tree = find(ET.fromstring(output.read(SLIDE)), "p:cSld/p:spTree")
        relations = {r.get("Id"): r for r in ET.fromstring(output.read(SLIDE_RELS))}
        original_layouts = [r.get("Target") for r in ET.fromstring(source.read(SLIDE_RELS))
                            if r.get("Type").endswith("/slideLayout")]
        new_layouts = [r.get("Target") for r in relations.values() if r.get("Type").endswith("/slideLayout")]
        assert new_layouts == original_layouts
        for name in expected_names:
            matches = [node for node in tree if (nv := node.find(".//p:cNvPr", NS)) is not None and nv.get("name") == name]
            assert len(matches) == 1, name
            node = matches[0]
            blips = node.findall(".//a:blip", NS)
            assert blips, name
            for blip in blips:
                target = relations[blip.get(tag("r:embed"))]
                assert target.get("TargetMode") != "External"
                assert posixpath.normpath(posixpath.join("ppt/slides", target.get("Target"))) == IMAGE
            if name != PLACEHOLDER:
                link = find(node, ".//a:hlinkClick")
                target = relations[link.get(tag("r:id"))]
                assert target.get("TargetMode") == "External" and target.get("Target") == WEB
            if name == GROUP:
                ids = [int(n.get("id")) for n in node.findall(".//p:cNvPr", NS)]
                assert len(ids) == len(set(ids)) == 4
                assert [int(find(node, f".//a:{end}").get("id")) for end in ("stCxn", "endCxn")] == ids[1:3]


def paired_report(folder, frozen, candidate):
    source, original = prepare(folder)
    models = {label: model(binary, original) for label, binary in (("frozen", frozen), ("candidate", candidate))}
    assert models["frozen"] == models["candidate"]
    before = models["frozen"]
    page = before["slides"][1]
    with ZipFile(source) as z:
        png = z.read(IMAGE)
    foreign = model(frozen, source)
    image = next(s for s in foreign["slides"][1]["shapes"] if s["kind"]["type"] == "picture")
    group = next(s for s in foreign["slides"][1]["shapes"] if s["name"] == GROUP)
    # This fixture supplies the portable payload directly to the CLI. The Windows
    # test separately exercises capture, document token checking and scope.
    group["click"] = None
    image["name"] = "Portable picture"
    resource = {"id": image["kind"]["fill"]["media"], "name": "copy.png", "content_type": "image/png", "data": list(png)}
    select = {"command": "select", "page": 2, "shapes": []}
    undo, redo = ({"command": id} for id in ("undo", "redo"))

    def paste(shape):
        return {"command": "paste-objects", "transfer": {"joydeck_objects": 1, "shapes": [shape], "media": [resource]}}

    def added(path, cli, names):
        opened = model(cli, path)
        shapes = opened["slides"][1]["shapes"]
        assert len(shapes) == len(page["shapes"]) + len(names)
        assert [s["name"] for s in shapes[-len(names):]] == names
        assert len(opened["media"]) == len(before["media"]) == 1
        assert layout_bindings(opened) == layout_bindings(before)
        return opened

    checked = {}
    binaries = {"frozen": frozen, "candidate": candidate}
    native = {}
    for label, cli in binaries.items():
        out = folder / label
        out.mkdir()
        commands = [select, paste(image), paste(image)]
        one = edit(cli, original, out, "one", [*commands, undo])
        assert added(one, cli, ["Portable picture"])["masters"] == before["masters"]
        undone = edit(cli, original, out, "undone", [*commands, undo, undo])
        assert model(cli, undone) == before
        native[label] = edit(cli, original, out, "pasted", [*commands, undo, redo])
        assert list(native_media(native[label]).values()) == [png]
        pub = edit(cli, original, out, "single-public", [select, paste(image), undo, redo], fmt="pptx")
        check_pptx(pub, original, ["Portable picture"])
        checked[label] = {"pictureShapes": 2, "pictureMedia": 1, "undoRedo": True}
    for writer in native.values():
        for reader in binaries.values():
            read = added(writer, reader, ["Portable picture", "Portable picture"])
            assert read == model(frozen, native["frozen"])

    rejected = edit(frozen, original, folder / "frozen", "group", [select, paste(group)], ok=False)
    assert "剪贴板" in rejected
    out = folder / "candidate"
    undone = edit(candidate, original, out, "group-undone", [select, paste(group), undo])
    assert model(candidate, undone) == before
    prepared = [select, paste(group), undo, redo]
    native_group = edit(candidate, original, out, "group", prepared)
    for cli in binaries.values():
        opened = added(native_group, cli, [GROUP])
        assert opened["masters"] == before["masters"]
        assert len(opened["slides"][1]["shapes"][-1]["kind"]["children"]) == 3
    assert list(native_media(native_group).values()) == [png]
    public_group = edit(candidate, original, out, "group", prepared, fmt="pptx")
    check_pptx(public_group, original, [GROUP])
    for cli in binaries.values():
        added(public_group, cli, [GROUP])

    return {
        "frozenSha256": sha(frozen),
        "candidateSha256": sha(candidate),
        "sourceSha256": sha(source),
        "destinationSha256": sha(original),
        "sharedPictureChecks": checked,
        "nativeCrossReadbacks": 4,
        "candidateGroupReadbacks": 4,
        "frozenGroupPasteRefused": True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    folder = args.folder.resolve()
    if args.frozen is None and args.candidate is None:
        prepare(folder)
        return
    if args.frozen is None or args.candidate is None:
        parser.error("--frozen and --candidate must be supplied together")
    report = paired_report(folder, args.frozen.resolve(), args.candidate.resolve())
    (folder / "cli-report.json").write_bytes((json.dumps(report, ensure_ascii=False, indent=2) + "\n").encode())
    print("Three transfer datasets and candidate group readback passed")


if __name__ == "__main__":
    main()
