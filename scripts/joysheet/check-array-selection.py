"""Check dynamic arrays through both XLSX generations and the native Sheet editor."""

import argparse
import csv
import io
import json
import math
import statistics
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

SOURCE = [("beta", 20), ("Alpha", 30), ("ALPHA", 10), ("gamma", 40), ("alpha", 30), ("beta", 5)]
FORMULAS = {
    "H1": '=FILTER(A1:B6,B1:B6>=F1,"空")',
    "K1": "=SORT(A1:B6,{1,2},{1,-1})",
    "N1": "=SORTBY(A1:B6,A1:A6,1,B1:B6,-1)",
    "Q1": "=UNIQUE(A1:A6)",
    "S1": "=UNIQUE(B1:B6,,TRUE)",
    "U1": "=UNIQUE(TRANSPOSE(A1:A6),TRUE,TRUE)",
    "W1": "=FILTER(A1:B6,{0,1})",
}


def where(clauses):
    return [
        item
        for name, args in clauses
        for item in ("--cmd", name + "=" + json.dumps(args, ensure_ascii=False))
    ]


def editor_commands(limit):
    commands = [
        ("range.setValues", {"range": "A1", "values": [[label, str(n)] for label, n in SOURCE]})
    ]
    return commands + [
        ("cell.set", {"cell": cell, "input": value})
        for cell, value in {"F1": str(limit), **FORMULAS}.items()
    ]


def read_csv(cli, *args):
    result = subprocess.run([str(cli), *map(str, args)], capture_output=True, check=True, timeout=180)
    return list(csv.reader(io.StringIO(result.stdout.decode())))


def expected(limit):
    # Each cell expected on the board is derived directly from the inputs.
    board = [[""] * 16 for _ in SOURCE]
    sorted_rows = sorted(SOURCE, key=lambda row: (row[0].lower(), -row[1]))

    def insert(offset, rows):
        for i, row in enumerate(rows):
            for j, cell in enumerate(row):
                board[i][offset + j] = str(cell)

    insert(0, [row for row in SOURCE if row[1] >= limit] or [["空"]])
    insert(3, sorted_rows)
    insert(6, sorted_rows)
    names = [s.lower() for s, n in SOURCE]
    numbers = [n for s, n in SOURCE]
    insert(9, [[s] for i, (s, n) in enumerate(SOURCE) if names.index(s.lower()) == i])
    insert(11, [[n] for n in numbers if numbers.count(n) == 1])
    insert(13, [[s for s, n in SOURCE if names.count(s.lower()) == 1]])
    insert(15, [[n] for n in numbers])
    return board


def saved_formulas(book, limit):
    with ZipFile(book) as z:
        sheet = ET.fromstring(z.read("xl/worksheets/sheet1.xml"))
    seen = {}
    for cell in sheet.findall(".//{*}c"):
        formula = cell.find("{*}f")
        if formula is None:
            continue
        origin = cell.get("r")
        assert formula.text.replace("_xlfn.", "").replace("_xlws.", "") == FORMULAS[origin][1:]
        seen[origin] = formula.get("ref")
    retained = sum(n >= limit for _, n in SOURCE)
    wanted = {
        "H1": f"H1:I{retained}",
        "K1": "K1:L6",
        "N1": "N1:O6",
        "Q1": "Q1:Q3",
        "S1": "S1:S4",
        "U1": "U1",
        "W1": "W1:W6",
    }
    assert seen == wanted, (seen, wanted)
    return seen


def native(binary, original, out):
    def launch(tag):
        number = port()
        profile = out / ("profile-" + tag)
        control = SheetControl(number, profile)
        return control, app(
            binary,
            ["--control", str(number), "--diagnostics"],
            out,
            {"JOYSHEET_CONFIG_DIR": str(profile)},
            tag + ".log",
            unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        )

    def board(client, limit):
        result = expected(limit)
        for i, row in enumerate(result, 1):
            for j, wanted in enumerate(row, ord("H")):
                addr = f"{chr(j)}{i}"
                cell = client.call("cell.get", cell=addr)
                actual = cell.get("value")
                expected_value = int(wanted) if wanted.isdecimal() else wanted or None
                assert actual == expected_value and cell["text"] == wanted, (
                    addr,
                    actual,
                    cell["text"],
                    wanted,
                )

    client, proc = launch("edit")
    original_sha = digest(original)
    saved = out / "动态数组 副本.xlsx"
    with proc as process:
        call = client.call
        client.wait(lambda v: v["perf"]["frame"] > 1, "desktop frame", process)

        def execute(command, **params):
            call("engine.execute", command=command, params=params)
            call("ui.awaitIdle")

        def edit(value):
            execute("edit.goTo", reference="F1")
            call("ui.cell.click", cell="F1")
            call("ui.edit.text", text=str(value))
            call("ui.edit.commit")
            call("ui.awaitIdle")

        call("app.open", path=str(original))
        call("ui.awaitIdle")
        board(client, 20)
        edit(35)
        board(client, 35)
        execute("edit.undo")
        board(client, 20)
        execute("edit.redo")
        board(client, 35)
        edit(99)
        board(client, 99)
        execute("cell.set", cell="H3", input="挡")
        edit(20)
        blocked = call("cell.get", cell="H1")
        assert "#SPILL!" in (blocked.get("display"), blocked.get("text"), blocked.get("value")), blocked
        assert call("cell.get", cell="H3")["value"] == "挡"
        execute("cell.set", cell="H3", input="")
        board(client, 20)
        edit(35)
        board(client, 35)
        call("app.save", path=str(saved))
    ranges = saved_formulas(saved, 35)
    client, proc = launch("reopen")
    with proc as process:
        client.wait(lambda v: v["perf"]["frame"] > 1, "fresh desktop", process)
        client.call("app.open", path=str(saved))
        client.call("ui.awaitIdle")
        board(client, 35)
    assert digest(original) == original_sha
    return {
        "binarySha256": digest(binary),
        "originalUnchanged": True,
        "undoRedo": True,
        "spillBlockAndRecovery": True,
        "checkedCellsPerState": 96,
        "verifiedStates": 8,
        "savedAndReopened": True,
        "spillRanges": ranges,
    }


def benchmark(versions, out, samples):
    rows = 4096
    records = [(f"group{(i * 31) % 113:03d}", (61 * i) % 47) for i in range(rows)]
    source = out / "array-perf.xlsx"
    commands = [
        (
            "range.setValues",
            {"range": f"A{i + 1}", "values": [[s, str(n)] for s, n in records[i : i + 512]]},
        )
        for i in range(0, rows, 512)
    ]
    numbers = f"B1:B{rows}"
    table = f"A1:B{rows}"
    names = f"A1:A{rows}"
    formulas = [
        f"=SUM(FILTER({numbers},{numbers}>=F1))",
        f"=INDEX(SORT(FILTER({table},{numbers}>=F1),{{1,2}},{{1,-1}}),1,2)",
        f"=INDEX(SORTBY({table},{numbers}*({numbers}>=F1),-1,{names},1),1,1)",
        f"=ROWS(UNIQUE(FILTER({names},{numbers}>=F1)))",
    ]
    commands.extend(
        [
            ("range.setValues", {"range": "D1", "values": [[f] for f in formulas]}),
            ("cell.set", {"cell": "F1", "input": "20"}),
        ]
    )
    read_csv(
        versions[0], "run", *where(commands), "--out", source, "--quiet", "--print", "D1:D4", "--csv"
    )
    # Every measurement opens the same workbook and commits six edits. Alternate
    # process order, verify all results, and keep every raw wall time.
    limits = [35, 20] * 3
    edits = where([("cell.set", {"cell": "F1", "input": str(v)}) for v in limits])
    filtered = [r for r in records if r[1] >= limits[-1]]
    correct = [
        str(sum(n for s, n in filtered)),
        str(sorted(filtered, key=lambda r: (r[0], -r[1]))[0][1]),
        sorted(records, key=lambda r: (-r[1] if r[1] >= limits[-1] else 0, r[0]))[0][0],
        str(len(set(s for s, n in filtered))),
    ]

    def sample(cli):
        start = time.perf_counter_ns()
        values = read_csv(cli, "run", "--in", source, *edits, "--quiet", "--print", "D1:D4", "--csv")
        elapsed = (time.perf_counter_ns() - start) / 1e6
        assert values == [[value] for value in correct], (values, correct)
        return round(elapsed, 4)

    for cli in versions:
        sample(cli)
    results = [[], []]
    for iteration in range(samples):
        for i in (0, 1) if iteration % 2 == 0 else (1, 0):
            results[i].append(sample(versions[i]))
    return {
        "rows": rows,
        "editsPerProcess": len(limits),
        "expected": correct,
        "timings": {
            key: {
                "binarySha256": digest(cli),
                "samplesMs": values,
                "p50Ms": statistics.median(values),
                "p95Ms": sorted(values)[math.ceil(samples * 0.95) - 1],
            }
            for cli, key, values in zip(versions, ("baseline", "candidate"), results)
        },
    }


def verify(old, new, desktop, out, release, samples):
    pairs = []
    for label, cli in (("old", old), ("new", new)):
        book = out / (label + ".xlsx")
        assert read_csv(
            cli,
            "run",
            *where(editor_commands(20)),
            "--out",
            book,
            "--quiet",
            "--print",
            "H1:W6",
            "--csv",
        ) == expected(20)
        saved_formulas(book, 20)
        for name, reader in (("old", old), ("new", new)):
            assert read_csv(reader, "cat", book, "--range", "H1:W6", "--csv") == expected(20)
            pairs.append([label, name])
    report = {
        "cliSha256": [digest(old), digest(new)],
        "formulas": FORMULAS,
        "writerReaderCombinations": pairs,
        "native": native(desktop, out / "old.xlsx", out),
    }
    if release:
        report["openAndSixArrayEdits"] = benchmark(release, out, samples)
    (out / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ("old", "new", "desktop", "output"):
        parser.add_argument(argument, type=Path)
    parser.add_argument("--release", type=Path, nargs=2)
    parser.add_argument("--samples", type=int, default=40)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    verify(
        args.old.resolve(),
        args.new.resolve(),
        args.desktop.resolve(),
        out,
        [p.resolve() for p in args.release or []],
        args.samples,
    )
