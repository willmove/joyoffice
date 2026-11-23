"""Compare worksheet default dimensions, hidden rows/cols, and outline levels across XLS, XLSB, and XLSX."""

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
DATA = Path(__file__).resolve().parent / "tests/data/sheet-dims"

# Expected per fixture:
#   default_row_pt (approx), default_col_gt_default (bool),
#   col_b (hidden, outline, collapsed, custom_width),
#   row_2 (hidden, outline, collapsed, custom_height, approx_ht_pt),
#   row_3 (hidden, outline, collapsed, has_ht)
CASES = {
    "grouped_outline.xls": (24.0, True, (True, 2, True, True), (True, 3, True, True, 30.0), (False, 1, False, False)),
    "grouped_outline.xlsb": (24.0, True, (True, 2, True, True), (True, 3, True, True, 30.0), (False, 1, False, False)),
    "grouped_outline.xlsx": (24.0, True, (True, 2, True, True), (True, 3, True, True, 30.0), (False, 1, False, False)),
    "custom_defaults.xls": (24.0, True, (False, 1, False, True), (False, 0, False, False, 15.0), (False, 0, False, False)),
    "custom_defaults.xlsb": (24.0, True, (False, 1, False, True), (False, 0, False, False, 15.0), (False, 0, False, False)),
    "custom_defaults.xlsx": (24.0, True, (False, 1, False, True), (False, 0, False, False, 15.0), (False, 0, False, False)),
}


def flag(node, attr, default=False):
    val = node.get(attr)
    if val is None:
        return default
    return val not in {"0", "false", "FALSE"}


def inspect_dims(file):
    with ZipFile(file) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheet = workbook.find(".//{*}sheet[@name='Details']")
        _, part = WORKBOOK["relations"](archive, "xl/workbook.xml")[sheet.get(WORKBOOK["REL"])]
        root = ET.fromstring(archive.read(part))
    fmt = root.find("{*}sheetFormatPr")
    def_row_pt = float(fmt.get("defaultRowHeight", "15"))
    def_col_w = fmt.get("defaultColWidth")
    def_col_gt = def_col_w is not None and float(def_col_w) > 10.0

    col_b = (False, 0, False, False)
    cols = root.find("{*}cols")
    if cols is not None:
        for c in cols.findall("{*}col"):
            cmin, cmax = int(c.get("min", "0")), int(c.get("max", "0"))
            if cmin <= 2 <= cmax:
                col_b = (
                    flag(c, "hidden"),
                    int(c.get("outlineLevel", "0")),
                    flag(c, "collapsed"),
                    flag(c, "customWidth"),
                )

    row_map = {}
    for r in root.findall("{*}sheetData/{*}row"):
        idx = int(r.get("r", "0"))
        ht = r.get("ht")
        row_map[idx] = (
            flag(r, "hidden"),
            int(r.get("outlineLevel", "0")),
            flag(r, "collapsed"),
            flag(r, "customHeight"),
            float(ht) if ht is not None else None,
        )

    r2 = row_map.get(2, (False, 0, False, False, None))
    r3 = row_map.get(3, (False, 0, False, False, None))
    return {
        "defaultRowPt": round(def_row_pt, 2),
        "defaultColCustom": def_col_gt,
        "colB": list(col_b),
        "row2": [r2[0], r2[1], r2[2], r2[3], round(r2[4], 2) if r2[4] is not None else None],
        "row3": [r3[0], r3[1], r3[2], r3[4] is not None],
        "outlineLevelRow": int(fmt.get("outlineLevelRow", "0")),
        "outlineLevelCol": int(fmt.get("outlineLevelCol", "0")),
    }


def matches_expected(actual, expected):
    def_row_pt, def_col_gt, exp_col_b, exp_r2, exp_r3 = expected
    assert abs(actual["defaultRowPt"] - def_row_pt) < 0.25, (actual, expected)
    assert actual["defaultColCustom"] is def_col_gt, (actual, expected)
    assert tuple(actual["colB"]) == exp_col_b, (actual, expected)
    assert tuple(actual["row2"][:4]) == exp_r2[:4], (actual, expected)
    assert actual["row2"][4] is not None and abs(actual["row2"][4] - exp_r2[4]) < 0.25, (actual, expected)
    assert tuple(actual["row3"]) == exp_r3, (actual, expected)


def exchange(book, old, new, output):
    expected = CASES[book.name]
    saved = {}
    for tag, writer in [("baseline", old), ("candidate", new)]:
        path = output / f"{book.name}-{tag}.xlsx"
        WORKBOOK["save"](writer, book, path, "A5", "dims marker")
        actual = inspect_dims(path)
        if tag == "candidate":
            matches_expected(actual, expected)
        saved[tag] = actual
        for reader_name, reader in [("baseline", old), ("candidate", new)]:
            reopened = output / f"{book.name}-{tag}-{reader_name}.xlsx"
            WORKBOOK["save"](reader, path, reopened, "D1", "reopen marker")
            if tag == reader_name == "candidate":
                assert inspect_dims(reopened) == actual, (book.name, inspect_dims(reopened), actual)
    return {"sourceSha256": digest(book), "writerDims": saved, "readerWriterPairs": 4}


def native(program, books, root):
    root.mkdir(parents=True, exist_ok=True)
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
                dest = root / ("尺寸-" + book.name + ".xlsx")
                call("app.open", path=str(book if phase == "edit" else dest))
                call("ui.awaitIdle")
                if phase == "reopen":
                    reopen_save = root / ("重开-" + book.name + ".xlsx")
                    call("app.save", path=str(reopen_save))
                    after = inspect_dims(reopen_save)
                    assert after["outlineLevelRow"] >= 1 and after["outlineLevelCol"] >= 1, (book.name, after)
                    continue
                # Group rows 5:6 and cols E:F, undo, verify undone, redo, save.
                call("engine.execute", command="data.group", params={"rows": "5:6"})
                call("engine.execute", command="data.group", params={"cols": "E:F"})
                call("ui.awaitIdle")
                call("engine.execute", command="edit.undo", params={})
                call("ui.awaitIdle")
                undo_dest = root / ("撤销-" + book.name + ".xlsx")
                call("app.save", path=str(undo_dest))
                undone = inspect_dims(undo_dest)
                matches_expected(undone, expected)
                call("engine.execute", command="edit.redo", params={})
                call("ui.awaitIdle")
                call("app.save", path=str(dest))
                saved = inspect_dims(dest)
                matches_expected(saved, expected)
                assert saved["outlineLevelCol"] >= max(expected[2][1], 1), (book.name, saved)
            logs[phase] = str(root / (phase + ".log"))
    return {"books": [b.name for b in books], "logs": logs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("old", type=Path)
    parser.add_argument("new", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    books = [DATA / name for name in sorted(CASES)]
    before = {b.name: digest(b) for b in books}
    result = {
        "cliSha256": [digest(args.old), digest(args.new)],
        "workbooks": {b.name: exchange(b, args.old, args.new, args.output) for b in books},
    }
    if args.desktop:
        result["native"] = native(args.desktop, books, args.output / "native")
    assert {b.name: digest(b) for b in books} == before
    result["originalsUnchanged"] = True
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
