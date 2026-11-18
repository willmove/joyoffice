"""Compare frozen panes, selection areas and live scroll positions across three Excel formats."""
import argparse
import json
import re
import runpy
import sys
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

WORKBOOK = runpy.run_path(str(Path(__file__).with_name("check-workbook-view.py")))
DATA = Path(__file__).resolve().parent / "tests/data/sheet-panes"
# Values selected in desktop Excel when each of the three source formats was saved.
CASES = {
    "quadrants": (3, 2, "E8", "bottomRight", "P16", "K11:M13 P16:Q17", 1),
    "rows": (4, 0, "D11", "bottomLeft", "D12", "D12:F15", 0),
    "columns": (0, 2, "G4", "topRight", "I7", "I7:J9", 0),
    "unfrozen": (0, 0, "D9", "topLeft", "Q21", "M14:N17 Q21:R23", 1),
}


def view(file):
    with ZipFile(file) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheet = workbook.find(".//{*}sheet[@name='Details']")
        _, part = WORKBOOK["relations"](archive, "xl/workbook.xml")[sheet.get(WORKBOOK["REL"])]
        node = ET.fromstring(archive.read(part)).find("{*}sheetViews/{*}sheetView")
    pane = node.find("{*}pane")
    attrs = pane.attrib if pane is not None else {}
    selected = {s.get("pane", "topLeft"): (s.get("activeCell", "A1"), s.get("sqref", "A1"), int(s.get("activeCellId", 0)))
                for s in node.findall("{*}selection")}
    active = attrs.get("activePane", "topLeft")
    rows, cols = int(attrs.get("ySplit", 0)), int(attrs.get("xSplit", 0))
    main = re.fullmatch(r"([A-Z]+)([0-9]+)", node.get("topLeftCell", "A1"))
    split = re.fullmatch(r"([A-Z]+)([0-9]+)", attrs.get("topLeftCell", "A1"))
    top = (split[1] if cols else main[1]) + (split[2] if rows else main[2])
    return (rows, cols, top, active,
            *selected.get(active, ("A1", "A1", 0))), selected


def exchange(book, old, new, output):
    expected = CASES[book.stem]
    saved = {}
    for tag, writer in [("baseline", old), ("candidate", new)]:
        path = output / f"{book.name}-{tag}.xlsx"
        WORKBOOK["save"](writer, book, path, "A3", "save marker")
        WORKBOOK["check_saved"](path, book, "Details", "Details", "save marker")
        actual, inactive = view(path)
        if tag == "candidate":
            assert actual == expected, (book, actual, expected)
            if book.stem == "quadrants":
                assert inactive["bottomLeft"] == ("A4", "A4", 0) and inactive["topRight"] == ("C1", "C1", 0)
        saved[tag] = actual
        for reader_name, reader in [("baseline", old), ("candidate", new)]:
            reopened = output / f"{book.name}-{tag}-{reader_name}.xlsx"
            WORKBOOK["save"](reader, path, reopened, "C1", "reopen marker")
            WORKBOOK["check_saved"](reopened, book, "Details", "Details", "save marker", reopened=True)
            if tag == reader_name == "candidate":
                assert view(reopened) == (actual, inactive)
    return {"sourceSha256": digest(book), "writerViews": saved, "readerWriterPairs": 4}


def address(row, col):
    label = ""
    while col >= 0:
        col, letter = divmod(col, 26)
        label = chr(65 + letter) + label
        col -= 1
    return label + str(row + 1)


def grid_origin(state, rows, cols):
    r = next((line["index"] for line in state["visibleRows"] if line["index"] >= rows), None)
    c = next((line["index"] for line in state["visibleColumns"] if line["index"] >= cols), None)
    return None if r is None or c is None else address(r, c)


def native(program, books, root):
    logs = {}
    for phase in ("edit", "reopen"):
        endpoint, profile = port(), root / (phase + "-profile")
        control = SheetControl(endpoint, profile)
        with app(program, ["--control", str(endpoint), "--diagnostics"], root, {"JOYSHEET_CONFIG_DIR": str(profile)},
                 phase + ".log", unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN")) as proc:
            control.wait(lambda s: s["perf"]["frame"] > 1, "native grid", proc)
            call = control.call
            for book in books:
                rows, cols, top, pane, active, areas, _ = CASES[book.stem]
                dest = root / ("窗格-" + book.name + ".xlsx")
                call("app.open", path=str(book if phase == "edit" else dest))
                call("ui.awaitIdle")
                expected_sel = areas.replace(" ", ",") if phase == "edit" else "AA50:AC51,AG56:AH57"
                expected_top = top if phase == "edit" else logs[book.name]["savedOrigin"]
                def matches(state):
                    return state["selection"] == expected_sel and grid_origin(state, rows, cols) == expected_top
                control.wait(matches, f"{book.name} {phase} expected {expected_sel} at {expected_top}", proc)
                def read():
                    return call("engine.execute", command="cell.get", params={"cell": active})["text"]
                if phase == "reopen":
                    assert read() == "原生窗口"
                    continue
                assert read() == ""
                call("ui.edit.text", text="原生窗口")
                assert call("ui.inspect")["editor"]["cell"] == active, book
                call("ui.edit.commit")
                call("ui.awaitIdle")
                assert read() == "原生窗口"
                for command, expected in [("edit.undo", ""), ("edit.redo", "原生窗口")]:
                    call("engine.execute", command=command, params={})
                    call("ui.awaitIdle")
                    assert read() == expected
                call("engine.execute", command="selection.set", params={"range": "AA50:AC51,AG56:AH57", "active": "AG56"})
                call("ui.awaitIdle")
                def changed(state):
                    return (state["selection"] == "AA50:AC51,AG56:AH57"
                            and 55 in {r["index"] for r in state["visibleRows"]}
                            and 32 in {c["index"] for c in state["visibleColumns"]})
                control.wait(changed, "scroll selected ranges into view", proc)
                new_top = grid_origin(call("ui.inspect"), rows, cols)
                assert new_top and new_top != top
                call("app.save", path=str(dest))
                assert view(dest)[0] == (rows, cols, new_top, pane, "AG56", "AA50:AC51 AG56:AH57", 1)
                logs[book.name] = {"originalOrigin": top, "editedCell": active, "savedOrigin": new_top}
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
    assert {p.name for p in books} == {f"{name}.{ext}" for name in CASES for ext in ("xls", "xlsb", "xlsx")}
    original = {book: digest(book) for book in books}
    result = {"cliSha256": [digest(p) for p in (args.baseline, args.candidate)],
              "workbooks": {book.name: exchange(book, args.baseline, args.candidate, output) for book in books}}
    if args.desktop:
        result["desktopSha256"] = digest(args.desktop)
        result["desktop"] = native(args.desktop.resolve(), books, output)
    assert all(digest(p) == sha for p, sha in original.items())
    result["originalsUnchanged"] = True
    (output / "report.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
