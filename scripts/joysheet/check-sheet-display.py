"""Compare worksheet display flags, view modes, zoom, and outline summary direction across XLS, XLSB, and XLSX."""

import argparse
import json
import runpy
import sys
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

WORKBOOK = runpy.run_path(str(Path(__file__).with_name("check-workbook-view.py")))
DATA = Path(__file__).resolve().parent / "tests/data/sheet-display"

# Expected (showFormulas, showGridLines, showHeadings, showZeros, showOutlineSymbols, rightToLeft, view, zoom, summaryBelow, summaryRight)
CASES = {
    "page_break.xls": (True, False, True, False, False, False, "pageBreakPreview", 85, False, True),
    "page_break.xlsb": (True, False, True, False, False, False, "pageBreakPreview", 85, False, True),
    "page_break.xlsx": (True, False, True, False, False, False, "pageBreakPreview", 85, False, True),
    "rtl_zoom.xls": (False, True, False, False, False, True, "normal", 154, True, False),
    "rtl_zoom.xlsb": (False, True, False, False, False, True, "normal", 154, True, False),
    "rtl_zoom.xlsx": (False, True, False, False, False, True, "normal", 154, True, False),
    "layout_outline.xls": (False, True, True, True, True, False, "normal", 135, False, False),
    "layout_outline.xlsb": (False, True, True, True, True, False, "pageLayout", 135, False, False),
    "layout_outline.xlsx": (False, True, True, True, True, False, "pageLayout", 135, False, False),
}


def flag(node, attr, default):
    val = node.get(attr)
    if val is None:
        return default
    return val not in {"0", "false", "FALSE"}


def inspect_display(file):
    with ZipFile(file) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheet = workbook.find(".//{*}sheet[@name='Details']")
        _, part = WORKBOOK["relations"](archive, "xl/workbook.xml")[sheet.get(WORKBOOK["REL"])]
        root = ET.fromstring(archive.read(part))
    view_node = root.find("{*}sheetViews/{*}sheetView")
    outline = root.find("{*}sheetPr/{*}outlinePr")
    view_mode = view_node.get("view", "normal")
    zoom = int(view_node.get("zoomScale", "100") or "100")
    scale_attr = {
        "normal": "zoomScaleNormal",
        "pageBreakPreview": "zoomScaleSheetLayoutView",
        "pageLayout": "zoomScalePageLayoutView",
    }[view_mode]
    mode_scale = int(view_node.get(scale_attr, str(zoom)))
    return (
        flag(view_node, "showFormulas", False),
        flag(view_node, "showGridLines", True),
        flag(view_node, "showRowColHeaders", True),
        flag(view_node, "showZeros", True),
        flag(view_node, "showOutlineSymbols", True),
        flag(view_node, "rightToLeft", False),
        view_mode,
        zoom,
        flag(outline, "summaryBelow", True) if outline is not None else True,
        flag(outline, "summaryRight", True) if outline is not None else True,
    ), (scale_attr, mode_scale)


def check_saved(path, edit, reopened=False):
    with ZipFile(path) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheets = workbook.findall("{*}sheets/{*}sheet")
        active_idx = int(workbook.find("{*}bookViews/{*}workbookView").get("activeTab", "0"))
        assert sheets[active_idx].get("name") == "Details", (path, active_idx)
        rels = WORKBOOK["relations"](archive, "xl/workbook.xml")
        _, part = rels[sheets[active_idx].get(WORKBOOK["REL"])]
        shared = []
        if "xl/sharedStrings.xml" in archive.namelist():
            shared = [
                "".join(t.text or "" for t in item.iterfind(".//{*}t"))
                for item in ET.fromstring(archive.read("xl/sharedStrings.xml"))
            ]
        grid = {}
        for cell in ET.fromstring(archive.read(part)).iterfind(".//{*}c"):
            val = cell.findtext("{*}v", "")
            if cell.get("t") == "s":
                val = shared[int(val)]
            elif cell.get("t") == "inlineStr":
                val = "".join(t.text or "" for t in cell.iterfind(".//{*}t"))
            grid[cell.get("r")] = val
        assert grid.get("A1") == "Details marker", (path, grid.get("A1"))
        assert grid.get("A3") == edit, (path, grid.get("A3"))
        if reopened:
            assert grid.get("C1") == "reopen marker", (path, grid.get("C1"))


def exchange(book, old, new, output):
    expected = CASES[book.name]
    saved = {}
    for tag, writer in [("baseline", old), ("candidate", new)]:
        path = output / f"{book.name}-{tag}.xlsx"
        WORKBOOK["save"](writer, book, path, "A3", "display marker")
        check_saved(path, "display marker")
        actual, scale_info = inspect_display(path)
        if tag == "candidate":
            assert actual == expected, (book.name, actual, expected)
            if expected[7] != 100:
                assert scale_info[1] == expected[7], (book.name, scale_info, expected)
        saved[tag] = list(actual)
        for reader_name, reader in [("baseline", old), ("candidate", new)]:
            reopened = output / f"{book.name}-{tag}-{reader_name}.xlsx"
            WORKBOOK["save"](reader, path, reopened, "C1", "reopen marker")
            check_saved(reopened, "display marker", reopened=True)
            if tag == reader_name == "candidate":
                assert inspect_display(reopened) == (actual, scale_info), (book.name, inspect_display(reopened))
    return {"sourceSha256": digest(book), "writerDisplay": saved, "readerWriterPairs": 4}


def native(program, books, root):
    logs = {}
    for phase in ("edit", "reopen"):
        endpoint, profile = port(), root / (phase + "-profile")
        control = SheetControl(endpoint, profile)
        with app(
            program,
            ["--control", str(endpoint), "--diagnostics"],
            root,
            {"JOYSHEET_CONFIG_DIR": str(profile)},
            phase + ".log",
            unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        ) as proc:
            control.wait(lambda s: s["perf"]["frame"] > 1, "native grid", proc)
            call = control.call
            for book in books:
                expected = CASES[book.name]
                dest = root / ("显示-" + book.name + ".xlsx")
                call("app.open", path=str(book if phase == "edit" else dest))
                call("ui.awaitIdle")
                if phase == "reopen":
                    outline = call("engine.execute", command="data.outlineSettings", params={})
                    assert outline["summaryBelow"] is (not expected[8]), (book.name, outline)
                    assert outline["summaryRight"] is (not expected[9]), (book.name, outline)
                    continue
                outline_before = call("engine.execute", command="data.outlineSettings", params={})
                assert outline_before == {"summaryBelow": expected[8], "summaryRight": expected[9]}, (book.name, outline_before, expected)
                # Flip gridlines, zoom, and outline summary direction, then undo/redo and save.
                flipped_grid = not expected[1]
                call("engine.execute", command="view.gridlines", params={"on": flipped_grid})
                call("engine.execute", command="view.zoom", params={"percent": 125})
                call(
                    "engine.execute",
                    command="data.outlineSettings",
                    params={"summaryBelow": not expected[8], "summaryRight": not expected[9]},
                )
                call("ui.awaitIdle")
                undo_dest = root / ("撤销-" + book.name + ".xlsx")
                call("engine.execute", command="edit.undo", params={})
                call("ui.awaitIdle")
                call("app.save", path=str(undo_dest))
                undone_disp, _ = inspect_display(undo_dest)
                assert (undone_disp[8], undone_disp[9]) == (expected[8], expected[9]), (book.name, undone_disp)
                call("engine.execute", command="edit.redo", params={})
                call("ui.awaitIdle")
                call("app.save", path=str(dest))
                saved_disp, _ = inspect_display(dest)
                want_saved = (
                    expected[0],
                    flipped_grid,
                    expected[2],
                    expected[3],
                    expected[4],
                    expected[5],
                    expected[6],
                    125,
                    not expected[8],
                    not expected[9],
                )
                assert saved_disp == want_saved, (book.name, saved_disp, want_saved)
                logs[book.name] = {"loadedSummary": outline_before, "savedZoom": 125, "savedGridlines": flipped_grid}
    return {"cases": logs, "undoRedo": True, "freshProcessReopen": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    books = sorted(DATA.glob("*.xls*"))
    assert {p.name for p in books} == set(CASES), ([p.name for p in books], sorted(CASES))
    original = {book: digest(book) for book in books}
    result = {
        "cliSha256": [digest(p) for p in (args.baseline, args.candidate)],
        "workbooks": {book.name: exchange(book, args.baseline, args.candidate, output) for book in books},
    }
    if args.desktop:
        result["desktopSha256"] = digest(args.desktop)
        result["desktop"] = native(args.desktop.resolve(), books, output)
    assert all(digest(p) == sha for p, sha in original.items())
    result["originalsUnchanged"] = True
    (output / "report.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
