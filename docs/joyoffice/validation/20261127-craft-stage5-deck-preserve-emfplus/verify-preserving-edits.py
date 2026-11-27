"""Exercise preserved chart/workbook cells and layout changes through both JoyDeck CLIs."""

import argparse
import hashlib
from io import BytesIO
import json
from pathlib import Path, PurePosixPath
import posixpath
import subprocess
from xml.etree import ElementTree as ET
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[4]
FIXTURES = ROOT / "tests/fixtures/joydeck"
R = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"
C = "{http://schemas.openxmlformats.org/drawingml/2006/chart}"
P = "{http://schemas.openxmlformats.org/presentationml/2006/main}"
MARKER = "urn:joydeck:vendor"
VALUE = 65.25
TITLE = "JoyDeck 原位标题"


def run(cli, *args):
    result = subprocess.run([str(cli.resolve()), *map(str, args)], check=True, capture_output=True,
                            encoding="utf-8", timeout=120)
    return json.loads(result.stdout)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def relationship(archive, owner, *, kind=None, rid=None):
    part = PurePosixPath(owner)
    rels = part.parent / "_rels" / (part.name + ".rels")
    nodes = ET.fromstring(archive.read(rels.as_posix()))
    edge, = [edge for edge in nodes
             if (kind is None or edge.get("Type").endswith("/" + kind))
             and (rid is None or edge.get("Id") == rid)]
    assert edge.get("TargetMode") != "External"
    target = edge.get("Target")
    path = target[1:] if target.startswith("/") else posixpath.normpath(str(part.parent / target))
    return path


def edit(cli, source, destination, actions):
    script = destination.with_suffix(".json")
    script.write_text(json.dumps(actions, ensure_ascii=False), encoding="utf-8")
    run(cli, "edit", source, "--script", script, "--output", destination)


def workbook_cell(book, cell):
    with ZipFile(BytesIO(book)) as archive:
        root = ET.fromstring(archive.read("xl/workbook.xml"))
        worksheet = root.find(".//{*}sheet")
        part = relationship(archive, "xl/workbook.xml", rid=worksheet.attrib[R])
        contents = ET.fromstring(archive.read(part))
        value = contents.find(f".//{{*}}c[@r='{cell}']/{{*}}v")
        return part, float(value.text)


def graph(path):
    with ZipFile(path) as archive:
        part = "ppt/slides/slide1.xml"
        slide = ET.fromstring(archive.read(part))
        chart = slide.find(".//" + C + "chart")
        chart_part = relationship(archive, part, rid=chart.attrib[R])
        root = ET.fromstring(archive.read(chart_part))
        cache = root.find(".//" + C + "val/" + C + "numRef/" + C + "numCache")
        point = cache.find(C + "pt[@idx='1']/" + C + "v")
        workbook = relationship(archive, chart_part, rid=root.find(C + "externalData").attrib[R])
        return chart_part, workbook, float(point.text)


def check_chart(frozen, candidate, folder):
    source = FIXTURES / "m9-powerpoint-edited-chart.pptx"
    original_chart, original_workbook, previous = graph(source)
    info = run(candidate, "inspect", source)
    chart_id, = (obj["id"] for obj in info["slides"][0]["objects"] if obj["kind"] == "chart")
    actions = [
        {"command": "select", "page": 1, "shapes": [chart_id]},
        {"command": "content", "action": {"kind": "chart-value", "series": 0, "index": 1, "value": VALUE}},
        {"command": "content", "action": {"kind": "chart-title", "title": TITLE}},
        {"command": "undo"},
        {"command": "redo"},
    ]
    marked = folder / "chart-marked.pptx"
    with ZipFile(source) as original, ZipFile(marked, "w") as output:
        for name in original.namelist():
            data = original.read(name)
            if name == original_chart:
                data = data.replace(b"<c:chartSpace ",
                                    b'<?vendor keep?><c:chartSpace xmlns:vendor="urn:joydeck:vendor" vendor:mark="retained" ', 1)
                assert b"<?vendor keep?>" in data
            output.writestr(name, data)
    snapshots = {}
    for label, fixture in (("chart-edited", source), ("chart-marked-edited", marked)):
        destination = folder / f"{label}.pptx"
        edit(candidate, fixture, destination, actions)
        chart_part, workbook, value = graph(destination)
        assert chart_part != original_chart and workbook != original_workbook and value == VALUE
        with ZipFile(fixture) as before, ZipFile(destination) as after:
            assert before.read(original_chart) == after.read(original_chart)
            book = before.read(original_workbook)
            assert book == after.read(original_workbook)
            updated = after.read(workbook)
            sheet, numeric = workbook_cell(updated, "B3")
            assert (workbook_cell(book, "B3")[1], numeric) == (previous, VALUE)
            with ZipFile(BytesIO(book)) as old, ZipFile(BytesIO(updated)) as new:
                assert set(old.namelist()) == set(new.namelist())
                assert [part for part in old.namelist() if old.read(part) != new.read(part)] == [sheet]
            chart_xml = after.read(chart_part)
            assert b"uniqueId" in chart_xml
            if fixture == marked:
                assert b"<?vendor keep?>" in chart_xml
                assert ET.fromstring(chart_xml).get("{" + MARKER + "}mark") == "retained"
            snapshots[label] = {"sha256": digest(destination.read_bytes()), "chart": chart_part,
                                "workbook": workbook, "sheet": sheet, "cell": "B3", "value": numeric}
        for cli in (frozen, candidate):
            model = run(cli, "content", destination)
            chart = next(shape["kind"] for shape in model["slides"][0]["shapes"] if shape["id"] == chart_id)
            assert chart["series"][0]["values"][1] == VALUE and chart["title"] == TITLE
    return snapshots


def check_layout(frozen, candidate, folder):
    source = FIXTURES / "m10-powerpoint-authored-motion.pptx"
    model = run(candidate, "content", source)
    slide = model["slides"][0]
    other = next(layout for layout in model["masters"][0]["layouts"] if layout["id"] != slide["layout"])
    actions = [{"command": "select", "page": 1, "shapes": []},
               {"command": "content", "action": {"kind": "layout", "id": other["id"]}}]
    part = "ppt/slides/slide1.xml"
    output = {}
    for label, cli in (("frozen", frozen), ("candidate", candidate)):
        path = folder / f"layout-{label}.pptx"
        edit(cli, source, path, actions)
        with ZipFile(source) as before, ZipFile(path) as after:
            source_layout = relationship(before, part, kind="slideLayout")
            layout = relationship(after, part, kind="slideLayout")
            assert source_layout != layout
            original_boxes = [ET.tostring(node) for node in ET.fromstring(before.read(part)).iter(P + "ph")]
            boxes = [ET.tostring(node) for node in ET.fromstring(after.read(part)).iter(P + "ph")]
            assert boxes and boxes == original_boxes
            for name in before.namelist():
                if name.startswith(("ppt/slideMasters/", "ppt/slideLayouts/")):
                    assert before.read(name) == after.read(name)
            output[label] = {"layout": layout, "placeholderCount": len(boxes)}
        result = run(candidate, "content", path)
        assert result["slides"][0]["layout"] == other["id"]
    assert output["frozen"] == output["candidate"]
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frozen", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    results = {"charts": check_chart(args.frozen, args.candidate, args.output),
               "layout": check_layout(args.frozen, args.candidate, args.output)}
    (args.output / "report.json").write_text(json.dumps(results, indent=2, ensure_ascii=False) + "\n",
                                            encoding="utf-8", newline="\n")
    print("Classic chart caches and embedded workbook, opaque chart metadata and layout inheritance verified")


if __name__ == "__main__":
    main()
