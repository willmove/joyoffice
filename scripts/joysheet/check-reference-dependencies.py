"""Create Excel reference data and check named ranges, database criteria and spreadsheet recalculation."""

import argparse
import csv
import io
import json
import os
from pathlib import Path
import math
import subprocess
import sys
import xml.etree.ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

DATABASE = [
    "DSUM",
    "DAVERAGE",
    "DCOUNT",
    "DCOUNTA",
    "DGET",
    "DMAX",
    "DMIN",
    "DPRODUCT",
    "DVAR",
    "DVARP",
    "DSTDEV",
    "DSTDEVP",
]
BASIC = [
    "SUM(Sales[Amount])",
    "LocalRate",
    "Rates!LocalRate",
    "GlobalRate",
    "RelativeRate",
    "SUM(Head:Tail)",
    "SUM(Sales[[First]:[Amount]])",
    "SUM(INDEX(Sales[Amount],2):Tail)",
    "SUM(INDIRECT(K1))",
    "SUM(OFFSET($B$2,$H$7,0,2,1))",
    "ROWS(Sales[#Data])",
    "INDEX(Sales[[#All],[Amount]],2)",
    "SUM(Sales[])",
    'DSUM(A1:B5,"Amount",E1:E2)',
    'DCOUNT(A1:B5,"Amount",E1:E2)',
    'DAVERAGE(A1:B5,"Amount",E1:E2)',
    'DGET(A1:B5,"Amount",E1:E2)',
    "DSUM(A1:B5,2,F1:F2)",
    "DCOUNT(A1:B5,2,F1:F2)",
    "DGET(A1:B5,2,F1:F2)",
    "DSUM(A1:B5,2,G1:G2)",
    "DSUM(A1:B5,2,I1:I2)",
    "DMAX(A1:B5,2,F1:F2)",
    "SUM(LocalRate,GlobalRate,DoubleAlias)",
    "SUM($B$2:INDEX(Sales[Amount],3))",
]


# Twelve database functions, numeric and text fields, empty/unique/multiple
# matches, mixed types, errors, compound criteria and omitted/malformed fields.
def formulas():
    tests = list(BASIC)
    for name in DATABASE:
        for field, criteria in [
            ('"Amount"', "S1:S2"),
            ('"Amount"', "T1:T2"),
            ('"Amount"', "U1:U2"),
            ('"Amount"', "V1:V3"),
            ('"Mixed"', "S1:S2"),
            ('"Mixed"', "T1:T2"),
            ('"Mixed"', "W1:W2"),
            ('"Mixed"', "X1:X2"),
            ('"Mixed"', "Y1:Y2"),
            ('"Mixed"', "Z1:Z2"),
            ('"Amount"', "AA1:AB3"),
            ("", "S1:S2"),
            ("3", "S1:S2"),
            ("9", "S1:S2"),
            ('"BadField"', "S1:S2"),
        ]:
            tests.append(f"{name}(N1:Q9,{field},{criteria})")
    return tests


def generate(path, variant):
    import xlsxwriter

    with xlsxwriter.Workbook(path) as book:
        tab = book.add_worksheet("Probe")
        data = book.add_worksheet("Rates")
        book.define_name("GlobalRate", "=Probe!$B$2")
        book.define_name("RelativeRate", "=Probe!$B1")
        book.define_name("Head", "=Probe!$B$2")
        book.define_name("Tail", "=Probe!$B$4")
        book.define_name("DoubleAlias", "=GlobalRate*2")
        book.define_name("LocalRate", "=Probe!$B$5")
        book.define_name("Probe!LocalRate", "=Probe!$B$3")
        book.define_name("Rates!LocalRate", "=Rates!$B$2")
        data.write("B2", 83)
        tab.add_table(
            "A1:C5",
            {
                "name": "Sales",
                "columns": [
                    {"header": "First"},
                    {"header": "Amount"},
                    {"header": "Fee"},
                ],
                "data": [
                    ["one", 12, 2],
                    ["two", 25, 3],
                    ["three", 5, 4],
                    ["four", 18, 5],
                ],
            },
        )
        tab.write("E1", "Match")
        tab.write_formula("E2", "=B2>H2")
        for col, expr in [
            ("F", '=B2>INDIRECT("H"&ROW(B2))'),
            ("G", "=IFERROR(ERROR.TYPE(H2)=4,FALSE)"),
            ("I", "=B2>H$2"),
        ]:
            tab.write(col + "1", "Computed")
            tab.write_formula(col + "2", expr)
        for row, value in enumerate([10, 5 if variant else 30, 5, 30], 1):
            tab.write(row, 7, value)
        tab.write("H7", 2 if variant > 1 else 0)
        tab.write("K1", "B4:B5" if variant > 1 else "B2:B3")
        tab.write("L1", -1)
        tab.set_selection("L1")
        for col, title in enumerate(["Kind", "Amount", "Mixed", "Seq"], 13):
            tab.write(0, col, title)
        for i, (kind, amount, mixed) in enumerate(
            [
                ("apple", 5, 12),
                ("apricot", -2.5, "text"),
                ("pear", 0, None),
                ("berry", 11, True),
                ("lemon", 7, "=NA()"),
                ("grape", 3, "=1/0"),
                ("apple", 4, 1.5),
                ("fig", 4, '=""'),
            ],
            1,
        ):
            tab.write(i, 13, kind)
            tab.write(i, 14, amount)
            tab.write(i, 15, mixed)
            tab.write(i, 16, i)
        for cell, value in {
            "S1": "Kind",
            "T1": "Kind",
            "T2": "pear",
            "U1": "Kind",
            "U2": "missing",
            "V1": "Kind",
            "V2": "ap",
            "V3": "fig",
            "W1": "Kind",
            "W2": "berry",
            "X1": "Kind",
            "X2": "lemon",
            "Y1": "Kind",
            "Y2": "grape",
            "Z1": "Kind",
            "Z2": "fig",
            "AA1": "Amount",
            "AB1": "Amount",
            "AA2": ">=3",
            "AB2": "<6",
            "AA3": "<0",
            "AB3": "<>0",
        }.items():
            tab.write(cell, value)
        for i, expr in enumerate(formulas(), 1):
            tab.write_formula(i, 9, "=" + expr)


def board(cli, book, env=None):
    result = subprocess.run(
        [
            cli,
            "cat",
            book,
            "--range",
            f"J2:J{len(formulas()) + 1}",
            "--sheet",
            "Probe",
            "--csv",
        ],
        env=env,
        capture_output=True,
        check=True,
        text=True,
    )
    return [r[0] if r else "" for r in csv.reader(io.StringIO(result.stdout))]


def cached(book):
    """Preserve Excel's cached type, including a computed empty string."""
    ns = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
    with ZipFile(book) as pkg:
        root = ET.fromstring(pkg.read("xl/worksheets/sheet1.xml"))
        cells = {c.get("r"): c for c in root.iter(ns + "c")}
        result = []
        for i in range(2, len(formulas()) + 2):
            cell = cells[f"J{i}"]
            formula = cell.find(ns + "f")
            assert formula is not None and formula.text
            kind = cell.get("t", "n")
            assert kind in {"n", "str", "e", "b"}, (i, kind)
            result.append((formula.text, kind, cell.findtext(ns + "v", default="")))
        return result


def expected(book):
    return [
        {"1": "TRUE", "0": "FALSE"}[v] if kind == "b" else v
        for _, kind, v in cached(book)
    ]


def equal(a, b, precision=1e-9):
    if a == b:
        return True
    try:
        return math.isclose(float(a), float(b), rel_tol=precision, abs_tol=precision)
    except ValueError:
        return False


def differences(actual, want):
    assert len(actual) == len(want)
    return [
        f"J{i + 2}" for i, (x, y) in enumerate(zip(actual, want)) if not equal(x, y)
    ]


def check_cache(actual, want):
    a = cached(actual)
    b = cached(want)
    assert len(a) == len(b)
    for i, ((expr, kind, value), (reference, typ, number)) in enumerate(zip(a, b)):
        assert (
            expr == reference
            and kind == typ
            and (equal(value, number, 1e-12) if typ == "n" else value == number)
        ), (i + 2, expr, kind, value, typ, number)


def exchange(old, new, book, out):
    original = digest(book)
    want = expected(book)
    prior = board(old, book)
    diff = differences(prior, want)
    assert diff and not differences(board(new, book), want), book
    conservative = board(
        new, book, {**os.environ, "JOYSHEET_REFERENCE_DEPENDENCIES": "0"}
    )
    assert not differences(conservative, want)
    for tag, writer in [("old", old), ("new", new)]:
        saved = out / f"{book.stem}-{tag}.xlsx"
        subprocess.run(
            [
                writer,
                "run",
                "--in",
                book,
                "--cmd",
                'cell.set={"cell":"L1","input":"exchange"}',
                "--out",
                saved,
                "--quiet",
            ],
            check=True,
            capture_output=True,
            timeout=120,
        )
        assert [f[0] for f in cached(saved)] == [f[0] for f in cached(book)]
        if tag == "new":
            check_cache(saved, book)
        for reader, values in [(old, prior), (new, want)]:
            assert not differences(board(reader, saved), values), (tag, str(reader))
    assert digest(book) == original
    return {
        "sha256": original,
        "formulas": len(want),
        "baselineDifferences": diff,
        "candidateDifferences": 0,
        "conservativeDifferences": 0,
        "typedSavedCaches": len(want),
        "oldNewReaderWriterPairs": 4,
    }


def desktop(program, books, out):
    def launch(tag):
        number = port()
        profile = out / (tag + "-profile")
        return SheetControl(number, profile), app(
            program,
            ["--control", str(number), "--diagnostics"],
            out,
            {"LC_ALL": "en_US.UTF-8", "JOYSHEET_CONFIG_DIR": str(profile)},
            tag + ".log",
            unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        )

    def edit(call, address, value):
        call("engine.execute", command="edit.goTo", params={"reference": address})
        call("ui.cell.click", cell=address)
        call("ui.edit.text", text=value)
        call("ui.edit.commit")
        call("ui.awaitIdle")

    def check(call, variant):
        for i, wanted in enumerate(boards[variant]):
            if i < len(BASIC) or i % 14 == 0:
                address = f"J{i + 2}"
                got = call("cell.get", cell=address)["text"]
                assert equal(got, wanted), (variant, address, got, wanted)

    sha = [digest(book) for book in books]
    boards = [expected(book) for book in books]
    saved = out / "名称与数据库重算.xlsx"
    client, session = launch("edit")
    with session as process:
        client.wait(lambda v: v["perf"]["frame"] > 1, "native Sheet window", process)
        call = client.call
        call("app.open", path=str(books[0]))
        call("ui.awaitIdle")
        check(call, 0)
        edit(call, "H3", "5")
        check(call, 1)
        call("engine.execute", command="edit.undo", params={})
        call("ui.awaitIdle")
        check(call, 0)
        call("engine.execute", command="edit.redo", params={})
        call("ui.awaitIdle")
        check(call, 1)
        edit(call, "K1", "B4:B5")
        edit(call, "H7", "2")
        check(call, 2)
        edit(call, "L8", "=SUM(Sales)")
        assert call("cell.get", cell="L8")["text"] == "74"
        call("app.save", path=str(saved))
    check_cache(saved, books[2])
    with ZipFile(saved) as pkg:
        root = ET.fromstring(pkg.read("xl/worksheets/sheet1.xml"))
        assert root.findtext(".//{*}c[@r='L8']/{*}f") == "SUM(Sales[])"
    client, session = launch("reopen")
    with session as process:
        client.wait(
            lambda v: v["perf"]["frame"] > 1, "fresh native Sheet window", process
        )
        client.call("app.open", path=str(saved))
        client.call("ui.awaitIdle")
        check(client.call, 2)
        assert client.call("cell.get", cell="L8")["text"] == "74"
    assert sha == [digest(book) for book in books]
    return {
        "binarySha256": digest(program),
        "states": 6,
        "cellsPerState": sum(
            i < len(BASIC) or i % 14 == 0 for i in range(len(formulas()))
        ),
        "editedComputedCriteria": [12, 37],
        "undoRedo": True,
        "savedTypedCaches": len(formulas()),
        "legacyBareTableInput": "SUM(Sales[])",
        "freshProcessReopen": True,
        "originalsUnchanged": True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="op", required=True)
    gen = sub.add_parser("generate")
    gen.add_argument("output", type=Path)
    gen.add_argument("variant", type=int, choices=range(3))
    verify = sub.add_parser("verify")
    for arg in ["baseline", "candidate", "output"]:
        verify.add_argument(arg, type=Path)
    verify.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    if args.op == "generate":
        generate(args.output, args.variant)
        return
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    old = args.baseline.resolve()
    new = args.candidate.resolve()
    books = [
        (
            Path(__file__).parent / "tests/data/reference-dependencies" / f"{i}.xlsx"
        ).resolve()
        for i in range(3)
    ]
    result = {
        "binaries": [digest(old), digest(new)],
        "workbooks": {b.name: exchange(old, new, b, out) for b in books},
    }
    if args.desktop:
        result["desktop"] = desktop(args.desktop.resolve(), books, out)
    (out / "report.json").write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
