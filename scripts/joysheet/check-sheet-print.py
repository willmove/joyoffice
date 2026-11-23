"""Compare worksheet print options, page margins, page setup, header/footer, and manual page breaks across XLS, XLSB, and XLSX."""

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
DATA = Path(__file__).resolve().parent / "tests/data/sheet-print"

# Expected:
#   (gridlines, headings, center_h, center_v,
#    margins_6, paper_size_id, scale, orientation, fit_to_pair_or_none,
#    header, footer, row_breaks, col_breaks)
CASES = {
    "landscape_a4.xls": (
        True, True, True, False,
        [0.5, 0.6, 0.8, 0.9, 0.35, 0.4], 9, 85, "landscape", None,
        "&CTitle", "Page &P", [10, 25], [4],
    ),
    "landscape_a4.xlsb": (
        True, True, True, False,
        [0.5, 0.6, 0.8, 0.9, 0.35, 0.4], 9, 85, "landscape", None,
        "&CTitle", "Page &P", [10, 25], [4],
    ),
    "landscape_a4.xlsx": (
        True, True, True, False,
        [0.5, 0.6, 0.8, 0.9, 0.35, 0.4], 9, 85, "landscape", None,
        "&CTitle", "Page &P", [10, 25], [4],
    ),
    "fit_breaks.xls": (
        False, True, False, True,
        [0.7, 0.7, 1.0, 1.0, 0.3, 0.3], 1, 100, "portrait", (2, 3),
        "&LReport", "&R&D", [12], [6, 12],
    ),
    "fit_breaks.xlsb": (
        False, True, False, True,
        [0.7, 0.7, 1.0, 1.0, 0.3, 0.3], 1, 100, "portrait", (2, 3),
        "&LReport", "&R&D", [12], [6, 12],
    ),
    "fit_breaks.xlsx": (
        False, True, False, True,
        [0.7, 0.7, 1.0, 1.0, 0.3, 0.3], 1, 100, "portrait", (2, 3),
        "&LReport", "&R&D", [12], [6, 12],
    ),
}


def flag(node, attr, default=False):
    if node is None:
        return default
    val = node.get(attr)
    if val is None:
        return default
    return val not in {"0", "false", "FALSE"}


def inspect_print(file):
    with ZipFile(file) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheet = workbook.find(".//{*}sheet[@name='Details']")
        _, part = WORKBOOK["relations"](archive, "xl/workbook.xml")[sheet.get(WORKBOOK["REL"])]
        root = ET.fromstring(archive.read(part))
    pr = root.find("{*}sheetPr/{*}pageSetUpPr")
    fit_flag = flag(pr, "fitToPage", False)
    po = root.find("{*}printOptions")
    pm = root.find("{*}pageMargins")
    ps = root.find("{*}pageSetup")
    hf = root.find("{*}headerFooter")
    margins = [round(float(pm.get(k, "0.75")), 3) for k in ("left", "right", "top", "bottom", "header", "footer")] if pm is not None else []
    paper = int(ps.get("paperSize", "1")) if ps is not None else 1
    scale = int(ps.get("scale", "100")) if ps is not None else 100
    orient = ps.get("orientation", "portrait") if ps is not None else "portrait"
    fit_to = (int(ps.get("fitToWidth", "1")), int(ps.get("fitToHeight", "1"))) if (fit_flag and ps is not None) else None
    header = hf.findtext("{*}oddHeader", "") if hf is not None else ""
    footer = hf.findtext("{*}oddFooter", "") if hf is not None else ""
    rbreaks = [int(b.get("id")) for b in root.findall("{*}rowBreaks/{*}brk")]
    cbreaks = [int(b.get("id")) for b in root.findall("{*}colBreaks/{*}brk")]
    return {
        "gridlines": flag(po, "gridLines", False),
        "headings": flag(po, "headings", False),
        "centerH": flag(po, "horizontalCentered", False),
        "centerV": flag(po, "verticalCentered", False),
        "margins": margins,
        "paperSize": paper,
        "scale": scale,
        "orientation": orient,
        "fitTo": list(fit_to) if fit_to is not None else None,
        "header": header,
        "footer": footer,
        "rowBreaks": rbreaks,
        "colBreaks": cbreaks,
    }


def matches_expected(actual, exp):
    assert actual["gridlines"] is exp[0], (actual, exp)
    assert actual["headings"] is exp[1], (actual, exp)
    assert actual["centerH"] is exp[2], (actual, exp)
    assert actual["centerV"] is exp[3], (actual, exp)
    assert all(abs(a - b) < 1e-3 for a, b in zip(actual["margins"], exp[4])), (actual, exp)
    assert actual["paperSize"] == exp[5], (actual, exp)
    assert actual["scale"] == exp[6], (actual, exp)
    assert actual["orientation"] == exp[7], (actual, exp)
    assert (tuple(actual["fitTo"]) if actual["fitTo"] is not None else None) == exp[8], (actual, exp)
    assert actual["header"] == exp[9], (actual, exp)
    assert actual["footer"] == exp[10], (actual, exp)
    assert actual["rowBreaks"] == exp[11], (actual, exp)
    assert actual["colBreaks"] == exp[12], (actual, exp)


def exchange(book, old, new, output):
    expected = CASES[book.name]
    saved = {}
    for tag, writer in [("baseline", old), ("candidate", new)]:
        path = output / f"{book.name}-{tag}.xlsx"
        WORKBOOK["save"](writer, book, path, "A5", "print marker")
        actual = inspect_print(path)
        if tag == "candidate":
            matches_expected(actual, expected)
        saved[tag] = actual
        for reader_name, reader in [("baseline", old), ("candidate", new)]:
            reopened = output / f"{book.name}-{tag}-{reader_name}.xlsx"
            WORKBOOK["save"](reader, path, reopened, "D1", "reopen marker")
            if tag == reader_name == "candidate":
                assert inspect_print(reopened) == actual, (book.name, inspect_print(reopened), actual)
    return {"sourceSha256": digest(book), "writerPrint": saved, "readerWriterPairs": 4}


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
                dest = root / ("打印-" + book.name + ".xlsx")
                call("app.open", path=str(book if phase == "edit" else dest))
                call("ui.awaitIdle")
                flipped_orient = "portrait" if expected[7] == "landscape" else "landscape"
                if phase == "reopen":
                    reopen_save = root / ("重开-" + book.name + ".xlsx")
                    call("app.save", path=str(reopen_save))
                    after = inspect_print(reopen_save)
                    assert after["orientation"] == flipped_orient, (book.name, after)
                    assert after["header"] == "&CEdited", (book.name, after)
                    continue
                call("engine.execute", command="pageLayout.pageSetup", params={"orientation": flipped_orient, "header": "&CEdited"})
                call("ui.awaitIdle")
                call("engine.execute", command="edit.undo", params={})
                call("ui.awaitIdle")
                undo_dest = root / ("撤销-" + book.name + ".xlsx")
                call("app.save", path=str(undo_dest))
                matches_expected(inspect_print(undo_dest), expected)
                call("engine.execute", command="edit.redo", params={})
                call("ui.awaitIdle")
                call("app.save", path=str(dest))
                saved = inspect_print(dest)
                assert saved["orientation"] == flipped_orient and saved["header"] == "&CEdited", (book.name, saved)
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
