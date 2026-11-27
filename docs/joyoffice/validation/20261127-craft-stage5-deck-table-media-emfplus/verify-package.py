"""Read edited tables and regenerated media directly from PPTX package members."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path, PurePosixPath
import subprocess
from urllib.parse import unquote
from xml.etree import ElementTree as ET
from zipfile import ZipFile, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[4]
SOURCE = ROOT / "tests/fixtures/joydeck"
SPEC = importlib.util.spec_from_file_location(
    "preservation", Path(__file__).resolve().parents[1]
    / "20261127-craft-stage5-deck-preserve-emfplus/verify-preserving-edits.py"
)
HELPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HELPER)

A = "{http://schemas.openxmlformats.org/drawingml/2006/main}"
R = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"
P14 = "{http://schemas.microsoft.com/office/powerpoint/2010/main}"
SLIDE = "ppt/slides/slide2.xml"
COLOR = "E6A857"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def table_snapshot(path, marker=False, require_style=True):
    with ZipFile(path) as archive:
        table = ET.fromstring(archive.read(SLIDE)).find(".//" + A + "tbl")
        first_row, second_row, *_ = table.findall(A + "tr")
        first, *_ = first_row.findall(A + "tc")
        anchor, covered = second_row.findall(A + "tc")[:2]
        assert anchor.get("gridSpan") == "2" and covered.get("hMerge") in ("1", "true")
        props, = first.findall(A + "tcPr")
        fill, = props.findall(A + "solidFill")
        color = fill.find(A + "srgbClr").get("val")
        assert color.upper() == COLOR
        text = first.find(".//" + A + "t").text
        assert text == "阶段 5 表格"
        bold = first.find(".//" + A + "rPr")
        assert bold.get("b") in ("1", "true")
        assert not marker or props.get("stage5-unmodeled") == "keep"
        style = table.find(A + "tblPr/" + A + "tableStyleId").text
        if require_style:
            styles_part = HELPER.relationship(archive, "ppt/presentation.xml", kind="tableStyles")
            styles = ET.fromstring(archive.read(styles_part))
            assert any(node.get("styleId") == style for node in styles.findall(A + "tblStyle"))
        snapshot = {"mergedColumns": 2, "fill": color, "bold": True, "text": text,
                    "styleId": style, "unknownAttribute": props.get("stage5-unmodeled")}
        if marker:
            anchor_props = anchor.find(A + "tcPr")
            assert anchor_props.get("marL") == "25400"
            assert anchor_props.find(A + "lnL").get("w") == "12700"
            snapshot.update({"mergedCellLeftMarginEmu": 25400, "mergedCellBorderWidthEmu": 12700})
        return snapshot


def media_snapshot(path, office_audio_stub=False):
    clips = []
    with ZipFile(path) as archive:
        for name in archive.namelist():
            if not (name.startswith("ppt/slides/slide") and name.endswith(".xml")):
                continue
            slide = ET.fromstring(archive.read(name))
            for picture in slide.findall(".//{http://schemas.openxmlformats.org/presentationml/2006/main}pic"):
                modern = picture.find(".//" + P14 + "media")
                legacy = picture.find(".//" + A + "videoFile")
                audio = legacy is None
                if audio:
                    legacy = picture.find(".//" + A + "audioFile")
                if modern is None or legacy is None:
                    continue
                embedded = unquote(HELPER.relationship(archive, name, rid=modern.attrib[R + "embed"]))
                link = legacy.attrib[R + "link"]
                if audio and office_audio_stub:
                    part = PurePosixPath(name)
                    relations = part.parent / "_rels" / (part.name + ".rels")
                    edge, = [edge for edge in ET.fromstring(archive.read(relations.as_posix()))
                             if edge.get("Id") == link]
                    assert edge.get("TargetMode") == "External" and edge.get("Target") == "NULL"
                    assert edge.get("Type").endswith("/audio")
                else:
                    target = unquote(HELPER.relationship(archive, name, rid=link))
                    assert target == embedded
                assert embedded in archive.namelist()
                poster = picture.find(".//" + A + "blip")
                poster_path = unquote(HELPER.relationship(archive, name, rid=poster.attrib[R + "embed"]))
                assert poster_path in archive.namelist()
                clips.append({"mediaSha256": digest(archive.read(embedded)),
                              "posterSha256": digest(archive.read(poster_path)),
                              "rolesShareTarget": not (audio and office_audio_stub)})
                if audio:
                    trim = modern.find(P14 + "trim")
                    clips[-1]["trimEndMs"] = int(trim.get("end")) if trim is not None else None
    assert clips
    return clips


def office_readback(table, video, legacy_audio, re_saved_audio):
    original, = media_snapshot(legacy_audio)
    saved, = media_snapshot(re_saved_audio, office_audio_stub=True)
    assert original | {"rolesShareTarget": False} == saved
    assert saved["trimEndMs"] == 5000
    return {"table": table_snapshot(table), "video": media_snapshot(video),
            "audioBeforeSave": original, "audioAfterSave": saved}


def marked_table(source, target, replace_cell):
    with ZipFile(source) as original, ZipFile(target, "w", ZIP_DEFLATED) as output:
        for name in original.namelist():
            data = original.read(name)
            if name == SLIDE:
                data = replace_cell(data)
            output.writestr(name, data)


def run(frozen, candidate, folder):
    folder.mkdir(parents=True)
    table = SOURCE / "independent.pptx"
    model = HELPER.run(candidate, "inspect", table)
    table_id, = (item["id"] for item in model["slides"][1]["objects"] if item["kind"] == "table")
    actions = [
        {"command": "select", "page": 2, "shapes": [table_id]},
        {"command": "table", "action": {"operation": "style", "name": "light1-accent3", "first_row": True, "band_row": True}},
        {"command": "set-text", "target": {"kind": "cell", "shape": table_id, "row": 0, "col": 0},
         "body": {"paragraphs": [{"runs": [{"text": "阶段 5 表格", "props": {"bold": True}}]}]}},
        {"command": "table", "action": {"operation": "fill", "row": 0, "col": 0, "rgb": int(COLOR, 16)}},
        {"command": "table", "action": {"operation": "merge", "row": 1, "col": 0, "end_row": 1, "end_col": 1}},
        {"command": "undo"}, {"command": "redo"},
    ]
    marked = folder / "unknown-cell.pptx"
    def sibling_metadata(data):
        cells = data.split(b"<a:tcPr/>")
        assert len(cells) == 7
        replacements = [b'<q:tcPr xmlns:q="http://schemas.openxmlformats.org/drawingml/2006/main" '
                        b'stage5-unmodeled="keep"><a:solidFill><a:srgbClr val="FFFFFF"/>'
                        b'</a:solidFill></q:tcPr>', b"<a:tcPr/>",
                        b'<a:tcPr marL="25400"><a:lnL w="12700"><a:noFill/></a:lnL></a:tcPr>',
                        *([b"<a:tcPr/>"] * 3)]
        return cells[0] + b"".join(tag + cell for tag, cell in zip(replacements, cells[1:]))

    marked_table(table, marked, sibling_metadata)
    tables = {}
    for label, source in (("table", table), ("table-with-opaque-cell", marked)):
        target = folder / f"{label}.pptx"
        HELPER.edit(candidate, source, target, actions)
        tables[label] = table_snapshot(target, marker=(source == marked))
        for cli in (frozen, candidate):
            inspected = HELPER.run(cli, "inspect", target)
            read = next(item["table"] for item in inspected["slides"][1]["objects"] if item["id"] == table_id)
            assert read["rows"][1]["cells"][0]["gridSpan"] == 2

    nested = folder / "nested-unknown-source.pptx"
    marked_table(table, nested, lambda data: data.replace(
        b"<a:tcPr/>", b'<a:tcPr><a:solidFill><a:srgbClr val="FFFFFF" stage5-unmodeled="nested"/>'
        b'</a:solidFill></a:tcPr>', 1))
    refused = folder / "nested-unknown-edited.pptx"
    result = subprocess.run([str(candidate.resolve()), "edit", str(nested), "--script",
                             str((folder / "table.json").resolve()), "--output", str(refused)],
                            capture_output=True, encoding="utf-8", timeout=120)
    assert result.returncode and "opaque formatting" in result.stderr and not refused.exists(), result
    media = SOURCE / "r6-r7/delivery-media.pptx"
    original_media = media_snapshot(media)
    media_evidence = {"original": original_media}
    for label, cli in (("frozen", frozen), ("candidate", candidate)):
        path = folder / f"media-{label}.pptx"
        HELPER.run(cli, "exchange", media, "--mode", "regenerate", "--output", path)
        media_evidence[label] = media_snapshot(path)
        assert [clip["mediaSha256"] for clip in media_evidence[label]] == [clip["mediaSha256"] for clip in original_media]
    assert media_evidence["frozen"] == media_evidence["candidate"]
    return {"tables": tables, "nestedUnknownFillEditRefused": True, "media": media_evidence}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = run(args.frozen, args.candidate, args.output)
    (args.output / "report.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n",
                                            encoding="utf-8", newline="\n")
    print("Table formatting, unknown cell XML, and regenerated audio/video/poster relationships verified")
