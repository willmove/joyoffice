"""Check generated ChartEx style relationships and chart caches against embedded XLSX cells."""

import argparse
import io
import hashlib
import json
import posixpath
import re
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile


C = "http://schemas.openxmlformats.org/drawingml/2006/chart"
CX = "http://schemas.microsoft.com/office/drawing/2014/chartex"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
S = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
STYLE = "http://schemas.microsoft.com/office/drawing/2012/chartStyle"
STYLE_TYPES = {
    "chartStyle": ("application/vnd.ms-office.chartstyle+xml", "chartStyle"),
    "chartColorStyle": ("application/vnd.ms-office.chartcolorstyle+xml", "colorStyle"),
}
RANGE = re.compile(r"(?:'Sheet1'|Sheet1)!\$([A-Z]+)\$(\d+)(?::\$([A-Z]+)\$(\d+))?")
CELL = re.compile(r"([A-Z]+)(\d+)")


def column_number(text):
    result = 0
    for letter in text:
        result = result * 26 + ord(letter) - ord("A") + 1
    return result


def cells(book):
    with ZipFile(io.BytesIO(book)) as archive:
        assert archive.testzip() is None
        sheet = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        assert workbook.find(f".//{{{S}}}sheet").attrib["name"] == "Sheet1"
        shared = []
        if "xl/sharedStrings.xml" in archive.namelist():
            strings = ET.fromstring(archive.read("xl/sharedStrings.xml"))
            shared = ["".join(text.text or "" for text in item.iter(f"{{{S}}}t")) for item in strings]
        result = {}
        for cell in sheet.iter(f"{{{S}}}c"):
            column, row = CELL.fullmatch(cell.attrib["r"]).groups()
            assert cell.find(f"{{{S}}}f") is None, cell.attrib["r"]
            value = cell.findtext(f"{{{S}}}v")
            if cell.get("t") == "inlineStr":
                value = "".join(text.text or "" for text in cell.iter(f"{{{S}}}t"))
            elif cell.get("t") == "s":
                value = shared[int(value)]
            elif value is not None:
                value = float(value)
            result[(column_number(column), int(row))] = value
        return result


def coordinates(formula):
    match = RANGE.fullmatch(formula)
    assert match, formula
    start, first, end, last = match.groups()
    return column_number(start), int(first), column_number(end or start), int(last or first)


def check_level(sheet, formula, level, namespace, numeric, level_index=0):
    left, first, right, last = coordinates(formula)
    column = right - level_index
    assert column >= left and last >= first
    count = (int(level.attrib["ptCount"]) if namespace == CX
             else int(level.find(f"{{{C}}}ptCount").attrib["val"]))
    points = {}
    for item in level.findall(f"{{{namespace}}}pt"):
        index = int(item.attrib["idx"])
        assert index not in points and 0 <= index < count
        value = (item.text if namespace == CX else item.findtext(f"{{{C}}}v")) or ""
        points[index] = float(value) if numeric and value else (None if numeric else value)
    assert count <= last - first + 1, (formula, count)
    for index in range(count):
        actual = sheet.get((column, first + index))
        expected = points.get(index)
        if numeric:
            assert actual == expected, (formula, index, expected, actual)
        else:
            assert (actual or "") == (expected or ""), (formula, index, expected, actual)
    return count


def relationships(archive, owner):
    folder, name = posixpath.split(owner)
    rels = posixpath.join(folder, "_rels", name + ".rels")
    if rels not in archive.namelist():
        return {}
    root = ET.fromstring(archive.read(rels))
    output = {}
    for node in root:
        rid, kind, target = [node.attrib[k] for k in ("Id", "Type", "Target")]
        assert rid not in output, owner
        if node.get("TargetMode") == "External":
            continue
        path = posixpath.normpath(posixpath.join(folder, target)) if not target.startswith("/") else target[1:]
        assert path in archive.namelist(), (owner, kind, path)
        output[rid] = kind.rsplit("/", 1)[-1], path
    return output


def check_file(path):
    reports = []
    with ZipFile(path) as archive:
        types = {node.get("PartName", "")[1:]: node.get("ContentType")
                 for node in ET.fromstring(archive.read("[Content_Types].xml"))}
        chart_types = {"chart", "chartex"}
        for name, mime in types.items():
            if mime.rsplit(".", 1)[-1].split("+")[0] not in chart_types:
                continue
            part = ET.fromstring(archive.read(name))
            extended = part.tag == f"{{{CX}}}chartSpace"
            links = relationships(archive, name)
            style_hashes = {}
            if extended:
                for kind, (mime, tag) in STYLE_TYPES.items():
                    [target] = [p for role, p in links.values() if role == kind]
                    assert types[target] == mime
                    body = archive.read(target)
                    assert ET.fromstring(body).tag == f"{{{STYLE}}}{tag}"
                    style_hashes[kind] = hashlib.sha256(body).hexdigest()
            ns = CX if extended else C
            data = part.find(f".//{{{ns}}}externalData")
            if data is None:
                reports.append({"part": name, "styles": style_hashes, "workbook": None})
                continue
            kind, workbook = links[data.attrib[f"{{{R}}}id"]]
            assert kind == "package", (name, kind)
            body = archive.read(workbook)
            sheet = cells(body)
            points = 0
            refs = 0
            if extended:
                for dimension in part.iter():
                    field = dimension.tag.rsplit("}", 1)[-1]
                    formula = dimension.findtext(f"{{{CX}}}f")
                    if not formula:
                        continue
                    if field in {"numDim", "strDim"}:
                        for level_index, level in enumerate(dimension.findall(f"{{{CX}}}lvl")):
                            points += check_level(sheet, formula, level, CX, field == "numDim", level_index)
                    elif field == "txData":
                        column, row, _, _ = coordinates(formula)
                        assert sheet.get((column, row)) == dimension.findtext(f"{{{CX}}}v")
                    else:
                        continue
                    refs += 1
            else:
                for form, cache in (("numRef", "numCache"), ("strRef", "strCache")):
                    for node in part.iter(f"{{{C}}}{form}"):
                        points += check_level(sheet, node.findtext(f"{{{C}}}f"),
                                              node.find(f"{{{C}}}{cache}"), C, form == "numRef")
                        refs += 1
            assert refs > 0, name
            reports.append({"part": name, "styles": style_hashes, "workbook": workbook,
                            "workbookSha256": hashlib.sha256(body).hexdigest(), "cachePoints": points,
                            "worksheetCells": len(sheet), "references": refs})
    assert reports, path
    return {"name": path.name, "charts": reports}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", type=Path, nargs="+")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = [check_file(path) for path in args.files]
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"{len(result)} chart packages verified against their dependencies and workbooks")


if __name__ == "__main__":
    main()
