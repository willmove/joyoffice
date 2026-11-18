"""Compare broadcast formulas against independent grids, two XLSX readers and the native Sheet editor."""

import argparse
import json
import math
import runpy
import statistics
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

helper = runpy.run_path(str(Path(__file__).with_name("check-array-selection.py")))
read_csv, where = helper["read_csv"], helper["where"]
RECT = "D1:T12"
FORMULAS = {
    "D1": "=SEQUENCE(3,1)*10+SEQUENCE(1,4,A1)",
    "I1": '=IF({TRUE;FALSE;TRUE},{"甲","乙","丙"},SEQUENCE(1,3,A1))',
    "M1": "=IFERROR({1,#N/A,#DIV/0!},{6;7})",
    "Q1": "=MAP(SEQUENCE(3,1),SEQUENCE(1,2,A1),LAMBDA(x,y,x*y))",
    "D6": "=SEQUENCE(2,3)+SEQUENCE(4,2)",
    "H6": "=MOD(SEQUENCE(3,1,A1),{2,3,4})",
    "L6": '=CHOOSE({1;2},{"红","绿"},{10,20})',
    "O6": '=SWITCH({1;2;3},1,{"同","次"},2,{5,6},"无")',
    "S6": '=IFS({TRUE;FALSE;FALSE},{"先","后"},{FALSE;TRUE;FALSE},{7,8},TRUE,"末")',
    "D11": "=EOMONTH(A1:A2,{0;1})",
    "H11": "=IFNA({1,#N/A},{2;4})",
    "L11": "=INDEX(MAP({2},{3},LAMBDA(x,y,x+y)),1,1)",
}


def grid(offset, previous=False):
    board = [[""] * 17 for _ in range(12)]

    def put(column, row, values):
        c0 = ord(column) - ord("D")
        for i, cells in enumerate(values):
            for j, value in enumerate(cells):
                board[row + i - 1][c0 + j] = str(value)

    put("D", 1, [[10 * i + offset + j for j in range(4)] for i in (1, 2, 3)])
    put("I", 1, [["甲", "乙", "丙"], list(range(offset, offset + 3)), ["甲", "乙", "丙"]])
    put("M", 1, [[1, 6, 6]] if previous else [[1, 6, 6], [1, 7, 7]])
    put(
        "Q",
        1,
        [[i * offset, i * (offset + 1)] for i in (1, 2, 3)]
        if previous
        else [[offset, "#N/A"], ["#N/A", "#N/A"], ["#N/A", "#N/A"]],
    )
    put(
        "D", 6, [[5 * r + 2 * c + 2 if r < 2 and c < 2 else "#N/A" for c in range(3)] for r in range(4)]
    )
    put("H", 6, [[(offset + i) % j for j in (2, 3, 4)] for i in range(3)])
    put("L", 6, [["红", "绿"], [10, 20]])
    put("O", 6, [["同", "次"]] if previous else [["同", "次"], [5, 6], ["无", "无"]])
    put("S", 6, [["先", "后"]] if previous else [["先", "后"], [7, 8], ["末", "末"]])
    put("D", 11, [["#VALUE!"], ["#VALUE!" if previous else 59]])
    put("H", 11, [[1, 2]] if previous else [[1, 2], [1, 4]])
    put("L", 11, [[5]])
    return board


def check_grid(actual, expected):
    assert len(actual) == len(expected)
    for i, (row, wanted) in enumerate(zip(actual, expected), 1):
        assert len(row) == len(wanted)
        for j, (got, value) in enumerate(zip(row, wanted), ord("D")):
            assert got == value, (f"{chr(j)}{i}", got, value)


def cmds(changes):
    return where([("cell.set", {"cell": cell, "input": str(value)}) for cell, value in changes])


def check_xml(book, previous=False):
    with ZipFile(book) as z:
        doc = ET.fromstring(z.read("xl/worksheets/sheet1.xml"))
    actual = {}
    for cell in doc.findall(".//{*}c"):
        f = cell.find("{*}f")
        if f is None:
            continue
        name = cell.get("r")
        text = f.text
        for prefix in ("_xlfn.", "_xlws.", "_xlpm."):
            text = text.replace(prefix, "")
        assert text == FORMULAS[name][1:], (name, text, FORMULAS[name])
        actual[name] = f.get("ref")
    assert set(actual) == FORMULAS.keys()
    for name, extent in {
        "D1": "D1:G3",
        "I1": "I1:K3",
        "M1": "M1:O1" if previous else "M1:O2",
        "Q1": "Q1:R3",
        "D6": "D6:F9",
        "H6": "H6:J8",
        "L6": "L6:M7",
        "O6": "O6:P6" if previous else "O6:P8",
        "S6": "S6:T6" if previous else "S6:T8",
        "D11": "D11:D12",
        "H11": "H11:I11" if previous else "H11:I12",
    }.items():
        assert actual[name] == extent, (name, actual[name], extent)
    return actual


def conditional_groups():
    path = Path(__file__).parent / "tests/data/broadcast-conditions.json"
    return json.loads(path.read_text())


def positions(cases):
    for i, case in enumerate(cases):
        row, column = 3 + 5 * (i // 5), 4 + 5 * (i % 5)
        yield row, column, case


def cached_values(book):
    # Use the raw cached values so number formats and localized date displays
    # do not affect the reference comparison.
    with ZipFile(book) as z:
        doc = ET.fromstring(z.read("xl/worksheets/sheet1.xml"))
        shared = []
        if "xl/sharedStrings.xml" in z.namelist():
            strings = ET.fromstring(z.read("xl/sharedStrings.xml"))
            shared = ["".join(e.text or "" for e in item.iterfind(".//{*}t")) for item in strings]
    values = {}
    for cell in doc.findall(".//{*}c"):
        kind = cell.get("t", "n")
        raw = cell.findtext("{*}v")
        if kind == "inlineStr":
            value = "".join(e.text or "" for e in cell.iterfind(".//{*}t"))
        elif raw is None:
            continue
        elif kind == "s":
            value = shared[int(raw)]
        elif kind == "b":
            value = raw == "1"
        elif kind == "n":
            value = float(raw)
        else:
            value = raw
        values[cell.get("r")] = value
    return values


def differing_cases(book, cases):
    values = cached_values(book)

    def same(a, b):
        if type(a) in (float, int) and type(b) in (float, int):
            return math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-12)
        return type(a) is type(b) and a == b

    failures = []
    for row, column, case in positions(cases):
        cells = case["cells"]
        for dr in range(4):
            for dc in range(4):
                address = f"{chr(64 + column + dc)}{row + dr}"
                want = cells[dr][dc] if dr < len(cells) and dc < len(cells[dr]) else None
                if not same(values.get(address), want):
                    failures.append(
                        {
                            "case": case["id"],
                            "cell": address,
                            "actual": values.get(address),
                            "expected": want,
                        }
                    )
    return failures


def compare_conditions(versions, out):
    reports = []
    for i, group in enumerate(conditional_groups()):
        values = dict(group["inputs"])
        for row, column, case in positions(group["cases"]):
            values[f"{chr(64 + column)}{row}"] = case["formula"]
        versions_result = {}
        for name, cli in zip(("baseline", "candidate"), versions):
            book = out / f"{name}-conditions-{i}.xlsx"
            read_csv(
                cli, "run", *cmds(values.items()), "--quiet", "--out", book, "--print", "A1", "--csv"
            )
            deviations = differing_cases(book, group["cases"])
            if name == "candidate":
                assert not deviations, deviations
            versions_result[name] = deviations
        reports.append({"formulas": len(group["cases"]), "cellDifferencesFromExcel": versions_result})
    return reports


def native(binary, book, out):
    def launch(label):
        number = port()
        profile = out / ("profile-" + label)
        client = SheetControl(number, profile)
        return client, app(
            binary,
            ["--control", str(number), "--diagnostics"],
            out,
            {"JOYSHEET_CONFIG_DIR": str(profile)},
            label + ".log",
            unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        )

    def verify(client, offset):
        cells = [
            [
                client.call("cell.get", cell=f"{chr(c)}{r}")["text"]
                for c in range(ord("D"), ord("T") + 1)
            ]
            for r in range(1, 13)
        ]
        check_grid(cells, grid(offset))

    sha = digest(book)
    saved = out / "广播 编辑.xlsx"
    client, p = launch("open")
    with p as proc:
        client.wait(lambda v: v["perf"]["frame"] > 1, "native frame", proc)
        call = client.call
        call("app.open", path=str(book))
        call("ui.awaitIdle")
        verify(client, 2)
        call("engine.execute", command="edit.goTo", params={"reference": "A1"})
        call("ui.cell.click", cell="A1")
        call("ui.edit.text", text="5")
        call("ui.edit.commit")
        call("ui.awaitIdle")
        verify(client, 5)
        for command, x in [("edit.undo", 2), ("edit.redo", 5)]:
            call("engine.execute", command=command, params={})
            call("ui.awaitIdle")
            verify(client, x)
        call("app.save", path=str(saved))
    refs = check_xml(saved)
    client, p = launch("reopen")
    with p as proc:
        client.wait(lambda v: v["perf"]["frame"] > 1, "fresh native frame", proc)
        client.call("app.open", path=str(saved))
        client.call("ui.awaitIdle")
        verify(client, 5)
    assert digest(book) == sha
    return {
        "desktopSha256": digest(binary),
        "snapshots": 5,
        "cellsPerSnapshot": 204,
        "nativeEditUndoRedoReopen": True,
        "sourceUnchanged": True,
        "savedFormulaRanges": refs,
    }


def timings(versions, out, rounds):
    rows = 4096
    book = out / "4096.xlsx"
    inputs = {
        "A1": 0,
        "D1": f"=SUM(SEQUENCE({rows},1)*10+SEQUENCE(1,8,A1))",
        "D2": f"=SUM(IF(SEQUENCE({rows},1)>A1,SEQUENCE(1,8),0))",
        "D3": f"=SUM(MOD(SEQUENCE({rows},1)+A1,SEQUENCE(1,8)))",
    }
    read_csv(
        versions[0], "run", *cmds(inputs.items()), "--quiet", "--out", book, "--print", "D1:D3", "--csv"
    )
    changes = [1, 2, 7, 5, 200, 19]
    edits = cmds(("A1", x) for x in changes)
    final = changes[-1]
    correct = [
        sum(10 * i + final + j for i in range(1, rows + 1) for j in range(8)),
        sum(range(1, 9)) * (rows - final),
        sum((i + final) % j for i in range(1, rows + 1) for j in range(1, 9)),
    ]

    def sample(cli):
        start = time.perf_counter_ns()
        values = read_csv(cli, "run", "--in", book, *edits, "--quiet", "--print", "D1:D3", "--csv")
        wall = (time.perf_counter_ns() - start) / 1e6
        assert values == [[str(v)] for v in correct], (values, correct)
        return round(wall, 4)

    for cli in versions:
        sample(cli)
    times = [[], []]
    for i in range(rounds):
        for j in (0, 1) if i % 2 == 0 else (1, 0):
            times[j].append(sample(versions[j]))
    return {
        "rows": rows,
        "columns": 8,
        "edits": changes,
        "expected": correct,
        "milliseconds": {
            label: {
                "samples": v,
                "p50": statistics.median(v),
                "p95": sorted(v)[math.ceil(rounds * 0.95) - 1],
            }
            for label, v in zip(("baseline", "candidate"), times)
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "desktop", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--samples", type=int, default=40)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    versions = [args.baseline.resolve(), args.candidate.resolve()]
    dest = args.output.resolve()
    dest.mkdir(parents=True, exist_ok=False)
    pairs = []
    for label, writer in zip(("old", "new"), versions):
        book = dest / (label + ".xlsx")
        check_grid(
            read_csv(
                writer,
                "run",
                *cmds({"A1": 2, **FORMULAS}.items()),
                "--quiet",
                "--out",
                book,
                "--print",
                RECT,
                "--csv",
            ),
            grid(2, previous=label == "old"),
        )
        check_xml(book, previous=label == "old")
        for reader, cli in zip(("old", "new"), versions):
            check_grid(
                read_csv(cli, "cat", book, "--range", RECT, "--csv"), grid(2, previous=reader == "old")
            )
            pairs.append([label, reader])
    result = {
        "cliSha256": [digest(c) for c in versions],
        "formulas": FORMULAS,
        "writerReaderPairs": pairs,
        "conditionalCases": compare_conditions(versions, dest),
        "native": native(args.desktop.resolve(), dest / "old.xlsx", dest),
        "benchmark": timings(versions, dest, args.samples),
    }
    (dest / "report.json").write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
