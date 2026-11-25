"""Exercise rich text edits and inheritance against two actual JoyDeck binaries."""

import argparse
from copy import deepcopy
import json
from pathlib import Path
import runpy
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from PIL import Image, ImageChops


COMMON = runpy.run_path(str(Path(__file__).with_name("check-stage2-transfer.py")))
FIXTURE, SLIDE, SLIDE_RELS, LAYOUT, MASTER, NS = (
    COMMON[key] for key in ("FIXTURE", "SLIDE", "SLIDE_RELS", "LAYOUT", "MASTER", "NS")
)
add, find, rel, tag, edit, run, model, sha, frame = (
    COMMON[key] for key in ("add", "find", "rel", "tag", "edit", "run", "model", "sha", "transform")
)
PAGE = 2
SHAPE = 191
LINK = "https://example.org/joydeck-rich-text"
UPDATED_LINK = "https://example.org/joydeck-rich-text/updated"
TITLE = "Rich Text QA"
MARKER = "urn:joyoffice:stage2-rich-text"
ET.register_namespace("qa", MARKER)
ET.register_namespace("mc", "http://schemas.openxmlformats.org/markup-compatibility/2006")


def text_box(tree, identifier, *, document=False):
    node = add(tree, "p:sp")
    nv = add(node, "p:nvSpPr")
    add(nv, "p:cNvPr", id=str(identifier), name=TITLE)
    add(nv, "p:cNvSpPr")
    add(add(nv, "p:nvPr"), "p:ph", type="body", idx="91")
    props = add(node, "p:spPr")
    if document:
        frame(props, 117, 51, 710, 394)
    body = add(node, "p:txBody")
    add(body, "a:bodyPr")
    add(body, "a:lstStyle")
    return node, body


def draw_run(paragraph, text, properties=None, color=None, link=None):
    run = add(paragraph, "a:r")
    if properties or color or link:
        style = add(run, "a:rPr", **(properties or {}))
        if color:
            add(add(style, "a:solidFill"), "a:srgbClr", val=color)
        if link:
            add(style, "a:hlinkClick", **{tag("r:id"): link})
    add(run, "a:t").text = text


def fixture(path, opaque=False):
    with ZipFile(FIXTURE) as source, ZipFile(path, "x") as output:
        parts = {name: ET.fromstring(source.read(name)) for name in (SLIDE, SLIDE_RELS, LAYOUT, MASTER)}
        slide = parts[SLIDE]
        slide.set("showMasterSp", "0")
        slide.set("{http://schemas.openxmlformats.org/markup-compatibility/2006}Ignorable", "qa")
        tree = find(slide, "p:cSld/p:spTree")
        for item in list(tree):
            if item.tag not in {tag("p:nvGrpSpPr"), tag("p:grpSpPr")}:
                tree.remove(item)
        node, body = text_box(tree, SHAPE, document=True)
        sentinel = add(add(node, "p:extLst"), "p:ext", uri="{9B43E9BC-29C8-441E-8DA5-B6470F47DB74}")
        ET.SubElement(sentinel, f"{{{MARKER}}}marker").text = "keep source shape XML"
        shape_level = add(find(body, "a:lstStyle"), "a:lvl1pPr")
        add(add(shape_level, "a:lnSpc"), "a:spcPct", val="125000")

        p = add(body, "a:p")
        draw_run(p, "Inherited style ")
        draw_run(p, "Original link", {"i": "0", "u": "none", "sz": "2000"}, link="rIdQaWeb")
        p = add(body, "a:p")
        props = add(p, "a:pPr", algn="l", marL="635000", indent="-280000")
        if opaque:
            props.set(f"{{{MARKER}}}keep", "opaque paragraph property")
        add(props, "a:buChar", char="•")
        draw_run(p, "A bullet 😀", {"b": "0", "sz": "2400"}, "B33633")
        for label in ("First number", "Second number"):
            p = add(body, "a:p")
            props = add(p, "a:pPr", algn="l", marL="635000", indent="-280000")
            add(props, "a:buAutoNum", type="arabicPeriod", startAt="4")
            draw_run(p, label)
        p = add(body, "a:p")
        draw_run(p, "Page ")
        fld = add(p, "a:fld", id="{B75A8A0E-51AA-47A6-BA51-EEE647CC1492}", type="slidenum")
        add(fld, "a:t").text = "2"
        rel(parts[SLIDE_RELS], "rIdQaWeb", "hyperlink", LINK, external=True)

        layout_tree = find(parts[LAYOUT], "p:cSld/p:spTree")
        _, layout_body = text_box(layout_tree, SHAPE)
        level = add(find(layout_body, "a:lstStyle"), "a:lvl1pPr", algn="ctr")
        add(level, "a:defRPr", sz="2600")
        add(layout_body, "a:p")

        master_tree = find(parts[MASTER], "p:cSld/p:spTree")
        master_body = next(find(s, "p:txBody") for s in master_tree.findall("p:sp", NS)
                           if (ph := s.find("p:nvSpPr/p:nvPr/p:ph", NS)) is not None and ph.get("type") == "body")
        list_style = find(master_body, "a:lstStyle")
        list_style.clear()
        level = add(list_style, "a:lvl1pPr")
        props = add(level, "a:defRPr", i="1")
        add(add(props, "a:solidFill"), "a:srgbClr", val="235AA4")
        style = find(parts[MASTER], "p:txStyles/p:bodyStyle")
        style.clear()
        level = add(style, "a:lvl1pPr")
        add(add(level, "a:defRPr", b="1"), "a:latin", typeface="Arial")

        for item in source.infolist():
            name = item.filename
            value = ET.tostring(parts[name], encoding="utf-8", xml_declaration=True) if name in parts else source.read(name)
            output.writestr(item, value)


def body(deck):
    return next(s["text"] for s in deck["slides"][PAGE - 1]["shapes"] if s["id"] == SHAPE)


def paragraph_text(paragraph):
    return "".join(run["text"] for run in paragraph["runs"])


def replacements(original, destination_slide):
    select = {"command": "select", "page": PAGE, "shapes": [SHAPE]}
    target = {"kind": "shape", "shape": SHAPE}
    def replace_text(start, end, text):
        return {"command": "edit-text", "target": target, "start": start, "end": end, "text": text}
    def set_body(body):
        return {"command": "set-text", "target": target, "body": body}

    character = deepcopy(original)
    original_link = character["paragraphs"][0]["runs"][1]
    original_link["text"] = "New link"
    original_link["props"].update(size=29, bold=True, underline="sng")
    original_link["props"]["link"]["action"]["url"] = UPDATED_LINK
    bullet = character["paragraphs"][1]["runs"][0]
    bullet["props"]["italic"] = True
    number = character["paragraphs"][2]["runs"][0]
    number["props"] = {"link": {"action": {"kind": "slide", "slide": destination_slide}}}
    # Exercise UTF-16 selection across the emoji and then replace the richer state.
    first_line = paragraph_text(original["paragraphs"][0])
    emoji = len(first_line) + 1 + len("A bullet ")
    editing = [select, replace_text(emoji, emoji + 2, "文本"), set_body(character)]

    combined = deepcopy(character)
    p = combined["paragraphs"][1]
    p["props"].update(align="right", marginLeft=60, indent=-10,
                      lineSpacing={"kind": "pct", "v": 1.5}, spaceAfter={"kind": "pts", "v": 12},
                      bullet={"kind": "autoNum", "scheme": "romanUcPeriod", "start_at": 7})
    combined["paragraphs"][2]["props"]["bullet"] = {"kind": "char", "char": "◆"}
    return {"typing": editing[:-1], "character": editing, "combined": editing + [set_body(combined)]}


def verify_model(doc, stage, destination):
    p = body(doc)["paragraphs"]
    assert paragraph_text(p[0]) == "Inherited style New link"
    assert paragraph_text(p[1]) == "A bullet 😀"
    assert p[0]["runs"][0].get("props") is None, "inherited style was flattened"
    props = p[0]["runs"][1]["props"]
    assert props["size"] == 29 and props["bold"] and props["underline"] == "sng" and not props["italic"]
    assert props["link"]["action"] == {"kind": "url", "url": UPDATED_LINK}
    assert p[2]["runs"][0]["props"]["link"]["action"] == {"kind": "slide", "slide": destination}
    assert p[4]["runs"][1]["kind"] == {"kind": "field", "field": "slidenum"}
    assert p[1]["runs"][0]["props"]["italic"]
    assert p[3]["props"]["bullet"]["kind"] == "autoNum"
    if stage == "character":
        assert p[1]["props"]["bullet"] == {"kind": "char", "char": "•"}
        assert p[2]["props"]["bullet"]["start_at"] == 4
    else:
        assert p[1]["props"]["bullet"] == {"kind": "autoNum", "scheme": "romanUcPeriod", "start_at": 7}
        assert p[1]["props"]["align"] == "right" and p[1]["props"]["lineSpacing"] == {"kind": "pct", "v": 1.5}
        assert p[1]["props"]["spaceAfter"] == {"kind": "pts", "v": 12}
        assert p[2]["props"]["bullet"] == {"kind": "char", "char": "◆"}


def verify_package(path, source, stage, writer):
    with ZipFile(path) as output, ZipFile(source) as original:
        assert set(output.namelist()) == set(original.namelist())
        assert all(output.read(name) == original.read(name) for name in original.namelist() if name not in {SLIDE, SLIDE_RELS})
        slide = ET.fromstring(output.read(SLIDE))
        relations_xml = ET.fromstring(output.read(SLIDE_RELS))
        relations = {entry.get("Id"): entry for entry in relations_xml}
        qualified = all(entry.tag == tag("rel:Relationship") for entry in relations_xml)
        assert qualified == (writer == "candidate"), (writer, "relationship namespace")
        assert slide.findtext(f".//{{{MARKER}}}marker") == "keep source shape XML"
        click = slide.findall(".//a:hlinkClick", NS)
        external, internal = (relations[link.get(tag("r:id"))] for link in click)
        assert external.get("TargetMode") == "External" and external.get("Target") == UPDATED_LINK
        assert internal.get("Target").endswith("slide3.xml") and internal.get("Type").endswith("/slide")
        assert find(slide, ".//a:fld").get("id") == "{B75A8A0E-51AA-47A6-BA51-EEE647CC1492}"
        if stage == "combined":
            assert find(slide, ".//a:buAutoNum").get("startAt") == "7"
            assert find(slide, ".//a:buChar").get("char") == "◆"


def render(cli, source, folder):
    run(cli, "render", source, "--output", folder, "--page", str(PAGE), "--scale", "2")
    return Image.open(folder / f"slide-{PAGE:04}.png").convert("RGB")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    args.folder.mkdir(parents=True, exist_ok=True)
    source = args.folder / "Rich Text QA.pptx"
    if not source.exists():
        fixture(source)
    if args.frozen is None and args.candidate is None:
        return
    if not args.frozen or not args.candidate:
        parser.error("both binaries are required to compare results")
    bins = {"frozen": args.frozen, "candidate": args.candidate}
    original = model(args.frozen, source)
    assert original == model(args.candidate, source)
    destination = original["slides"][2]["id"]
    commands = replacements(body(original), destination)
    base = [render(binary, source, args.folder / f"{key}-original") for key, binary in bins.items()]
    assert ImageChops.difference(*base).getbbox() is None
    palette = base[0].getcolors(maxcolors=base[0].width * base[0].height)
    assert palette and any(color == (0x23, 0x5A, 0xA4) for _, color in palette), "master placeholder color did not reach raster"

    report = {"fixtureSha256": sha(source), "binaries": {key: sha(binary) for key, binary in bins.items()},
              "originalRasterPixelIdentical": True, "modelReadbacks": 0, "historyComparisons": 0,
              "preservedPptx": 0, "typedUnicodeReadbacks": 0, "combinedPptxAcceptedByCandidate": False,
              "candidateLinksUseOpcNamespace": True}
    states = {
        "character": ("character", commands["character"]), "combined": ("combined", commands["combined"]),
        "undo": ("character", commands["combined"] + [{"command": "undo"}]),
        "redo": ("combined", commands["combined"] + [{"command": "undo"}, {"command": "redo"}]),
    }
    snapshots = {}
    formatted_rasters = []
    for writer, binary in bins.items():
        typed = edit(binary, source, args.folder, f"{writer}-typing", commands["typing"])
        for reader in bins.values():
            paragraphs = body(model(reader, typed))["paragraphs"]
            assert paragraph_text(paragraphs[1]) == "A bullet 文本"
            assert paragraphs[0] == body(original)["paragraphs"][0]
            report["typedUnicodeReadbacks"] += 1
        for label, (stage, script) in states.items():
            for ext in ("joydeck", "pptx"):
                if (writer, stage, ext) == ("frozen", "combined", "pptx"):
                    assert "paragraph formatting edits" in edit(binary, source, args.folder, f"{writer}-{label}", script, fmt=ext, ok=False)
                    continue
                saved = edit(binary, source, args.folder, f"{writer}-{label}", script, fmt=ext)
                if ext == "pptx":
                    verify_package(saved, source, stage, writer)
                    report["preservedPptx"] += 1
                    report["combinedPptxAcceptedByCandidate"] |= stage == "combined"
                    if label == "character":
                        formatted_rasters.append(render(binary, saved, args.folder / f"{writer}-formatted"))
                for reader, cli in bins.items():
                    readback = model(cli, saved)
                    verify_model(readback, stage, destination)
                    report["modelReadbacks"] += 1
                    key = (writer, reader, ext, stage)
                    if label in {"undo", "redo"}:
                        assert snapshots[key] == readback, key
                        report["historyComparisons"] += 1
                    else:
                        snapshots[key] = readback
    assert ImageChops.difference(*formatted_rasters).getbbox() is None
    report["compatibleEditedRasterPixelIdentical"] = True

    opaque = args.folder / "Opaque paragraph.pptx"
    fixture(opaque, opaque=True)
    rejection = edit(args.candidate, opaque, args.folder, "opaque-rejection", commands["combined"], fmt="pptx", ok=False)
    assert "opaque formatting" in rejection
    report["opaqueParagraphProtected"] = True
    (args.folder / "rich-text-verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"OK: {report['modelReadbacks']} reads, {report['historyComparisons']} exact history comparisons, {report['preservedPptx']} preserved PPTX files")


if __name__ == "__main__":
    main()
