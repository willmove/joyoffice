"""Verify saved worksheet selection, chart passthrough and edits across XLS, XLSB and XLSX."""

import argparse
import hashlib
import json
from pathlib import Path
import posixpath
import subprocess
import sys
from xml.etree import ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port


DATA = Path(__file__).resolve().parent / "tests/data/workbook-view"
WORKSHEETS = {"Start": 10, "Archive": 20, "Summary": 30, "Details": 40}
REL = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id"


def relations(archive, part):
    parent, name = posixpath.split(part)
    path = posixpath.join(parent, "_rels", name + ".rels")
    if path not in archive.namelist():
        return {}
    return {
        rel.get("Id"): (rel.get("Type").rsplit("/", 1)[-1],
                        posixpath.normpath(posixpath.join(parent, rel.get("Target"))).lstrip("/"))
        for rel in ET.fromstring(archive.read(path))
        if rel.get("TargetMode") != "External"
    }


def chart_parts(archive, part):
    found = []
    for kind in ("chartsheet", "drawing", "chart"):
        found.append(hashlib.sha256(archive.read(part)).hexdigest())
        if kind != "chart":
            next_kind = "drawing" if kind == "chartsheet" else "chart"
            (part,) = [target for role, target in relations(archive, part).values() if role == next_kind]
    return found


def inspect(path):
    with ZipFile(path) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheet_list = workbook.findall("{*}sheets/{*}sheet")
        where = int(workbook.find("{*}bookViews/{*}workbookView").get("activeTab"))
        rels = relations(archive, "xl/workbook.xml")
        shared = []
        if "xl/sharedStrings.xml" in archive.namelist():
            shared = ["".join(t.text or "" for t in item.iterfind(".//{*}t"))
                      for item in ET.fromstring(archive.read("xl/sharedStrings.xml"))]
        tabs, chart, notes = [], [], {}
        for sheet in sheet_list:
            name = sheet.get("name")
            kind, part = rels[sheet.get(REL)]
            tabs.append(name)
            assert sheet.get("state", "visible") == ("hidden" if name == "Start" else "visible")
            if kind == "chartsheet":
                assert name == "Graph"
                chart = chart_parts(archive, part)
                continue
            assert kind == "worksheet" and name in WORKSHEETS
            grid = {}
            for cell in ET.fromstring(archive.read(part)).iterfind(".//{*}c"):
                value = cell.findtext("{*}v", "")
                if cell.get("t") == "s":
                    value = shared[int(value)]
                elif cell.get("t") == "inlineStr":
                    value = "".join(t.text or "" for t in cell.iterfind(".//{*}t"))
                grid[cell.get("r")] = (value, cell.findtext("{*}f"))
            amount = WORKSHEETS[name]
            assert grid["A1"] == (name + " marker", None)
            assert grid["B1"] == (str(amount), None) and grid["B2"][0] == str(2 * amount)
            notes[name] = (grid.get("A3", ("", None))[0], grid.get("C1", ("", None))[0], grid["B2"][1])
        return tabs[where], tabs, chart, notes


def check_saved(path, source, active, edited_sheet, edit, reopened=False):
    actual, tabs, chart, cells = inspect(path)
    binary = source.suffix != ".xlsx"
    assert actual == active, (source, active, actual)
    assert tabs == (list(WORKSHEETS) if binary else ["Start", "Archive", "Graph", "Summary", "Details"])
    assert chart == ([] if binary else inspect(source)[2])
    for name, (mark, second, formula) in cells.items():
        assert mark == (edit if name == edited_sheet else ""), (source, edited_sheet, name, mark)
        assert second == ("reopen marker" if reopened and name == active else ""), (source, name, second)
        assert formula == (None if source.suffix == ".xlsb" else "B1*2"), (source, name, formula)


def save(cli, book, output, cell, mark):
    change = json.dumps({"cell": cell, "input": mark})
    subprocess.run([cli, "run", "--in", book, "--cmd", "cell.set=" + change, "--out", output, "--quiet"],
                   check=True, capture_output=True, timeout=40)


def expected(book):
    return "Details" if book.stem == "details" else "Archive"


def exchange(old, new, book, folder):
    before = digest(book)
    name = expected(book)
    observed = {}
    for tag, writer in (("baseline", old), ("candidate", new)):
        output = folder / f"{book.name}-{tag}.xlsx"
        save(writer, book, output, "A3", "save marker")
        actual, _, _, cells = inspect(output)
        (edited_sheet,) = [sheet for sheet, (mark, _, _) in cells.items() if mark == "save marker"]
        check_saved(output, book, name if tag == "candidate" else actual,
                    name if tag == "candidate" else edited_sheet, "save marker")
        observed[tag] = {"savedTab": actual, "editedWorksheet": edited_sheet}
        for reader_tag, reader in (("baseline", old), ("candidate", new)):
            reopened = folder / f"{book.name}-{tag}-{reader_tag}.xlsx"
            save(reader, output, reopened, "C1", "reopen marker")
            check_saved(reopened, book, actual, edited_sheet, "save marker", reopened=True)
    assert digest(book) == before
    return {"sourceSha256": before, "opened": observed, "readerWriterPairs": 4}


def native(program, books, root):
    def launch(tag):
        address, profile = port(), root / (tag + "-profile")
        server = app(program, ["--control", str(address), "--diagnostics"], root,
                     {"JOYSHEET_CONFIG_DIR": str(profile)}, tag + ".log",
                     unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"))
        return SheetControl(address, profile), server

    def verify(call, book, mark):
        call("ui.awaitIdle")
        def read(cell):
            return call("engine.execute", command="cell.get", params={"cell": cell})
        name = expected(book)
        assert read("A1")["text"] == name + " marker", (book, name)
        assert read("B2")["value"] == 2 * WORKSHEETS[name]
        assert read("A3")["text"] == mark, (book, mark)

    original = {book: digest(book) for book in books}
    outputs = {book: root / (book.name + "-window.xlsx") for book in books}
    for phase in ("edit", "reopen"):
        control, session = launch(phase)
        with session as proc:
            control.wait(lambda s: s["perf"]["frame"] > 1, "window opened", proc)
            call = control.call
            for book in books:
                output = outputs[book]
                call("app.open", path=str(book if phase == "edit" else output))
                if phase == "reopen":
                    verify(call, book, "window marker")
                    continue
                verify(call, book, "")
                call("ui.cell.click", cell="A3")
                call("ui.edit.text", text="window marker")
                call("ui.edit.commit")
                verify(call, book, "window marker")
                for command, mark in (("edit.undo", ""), ("edit.redo", "window marker")):
                    call("engine.execute", command=command, params={})
                    verify(call, book, mark)
                call("app.save", path=str(output))
                check_saved(output, book, expected(book), expected(book), "window marker")
    assert all(digest(book) == original[book] for book in books)
    return {"files": len(books), "statesPerFile": 5, "freshProcessReopen": True,
            "onlyActiveWorksheetModified": True, "sourceHashesUnchanged": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    folder = args.output.resolve()
    folder.mkdir(parents=True, exist_ok=True)
    books = sorted(DATA.glob("*.xls*"))
    assert {p.name for p in books} == {f"{s}.{ext}" for s in ("details", "chart") for ext in ("xlsx", "xlsb", "xls")}
    result = {"binaries": [digest(p) for p in (args.baseline, args.candidate)],
              "workbooks": {book.name: exchange(args.baseline, args.candidate, book, folder) for book in books}}
    if args.desktop:
        result["desktopSha256"] = digest(args.desktop)
        result["native"] = native(args.desktop.resolve(), books, folder)
    (folder / "report.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
