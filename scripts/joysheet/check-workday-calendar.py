"""Check Excel workday caches, binary scalar import and native holiday edits."""

import argparse
import csv
import io
import json
import math
import os
from pathlib import Path
import subprocess
import sys
from xml.etree import ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

DATA = Path(__file__).resolve().parent / "tests/data/workday-calendar"
REGIONS = {"en": "en_US.UTF-8", "zh": "zh_CN.UTF-8"}


def cells(path):
    with ZipFile(path) as z:
        sheet = ET.fromstring(z.read("xl/worksheets/sheet1.xml"))
    result = []
    for cell in sheet.iterfind(".//{*}c"):
        addr = cell.attrib["r"]
        expression = cell.findtext("{*}f")
        if addr.startswith("G") and addr[1:].isdigit() and expression:
            assert addr == f"G{len(result) + 2}"
            kind = cell.get("t", "n")
            assert kind in {"n", "e"}
            value = cell.findtext("{*}v")
            assert value is not None
            result.append((expression.removeprefix("_xlfn."), kind, value))
    assert result
    return result


def csv_range(cli, book, address, language):
    p = subprocess.run(
        [cli, "cat", book, "--range", address, "--sheet", "Probe", "--csv"],
        env={**os.environ, "LC_ALL": REGIONS[language]},
        check=True,
        capture_output=True,
        text=True,
        timeout=60,
    )
    return list(csv.reader(io.StringIO(p.stdout)))


def matches(a, b):
    if a == b:
        return True
    try:
        return math.isclose(float(a), float(b), rel_tol=3e-15, abs_tol=0)
    except ValueError:
        return False


def board(cli, book, rows, language):
    grid = csv_range(cli, book, f"G2:G{len(rows) + 1}", language)
    assert len(grid) == len(rows) and all(len(row) == 1 for row in grid)
    return [row[0] for row in grid]


def save(cli, book, dest, language):
    subprocess.run(
        [
            cli,
            "run",
            "--in",
            book,
            "--cmd",
            'cell.set={"cell":"L1","input":"exchange"}',
            "--out",
            dest,
            "--quiet",
        ],
        env={**os.environ, "LC_ALL": REGIONS[language]},
        check=True,
        capture_output=True,
        timeout=60,
    )


def calendar(old, new, book, out):
    language = "en" if "regional-en" in book.name else "zh"
    initial = digest(book)
    rows = cells(book)
    want = [v for _, _, v in rows]
    before = board(old, book, rows, language)
    wrong = sum(not matches(a, b) for a, b in zip(before, want))
    assert all(matches(a, b) for a, b in zip(board(new, book, rows, language), want)), (
        book
    )
    for tag, writer in [("old", old), ("new", new)]:
        output = out / f"{book.stem}-{tag}.xlsx"
        save(writer, book, output, language)
        written = cells(output)
        assert len(written) == len(rows)
        for (f, t, v), (expr, kind, val) in zip(written, rows):
            assert f == expr and (tag == "old" or (t == kind and matches(v, val))), (
                book,
                f,
                expr,
                t,
                kind,
                v,
                val,
            )
        for reader, expected in [(old, before), (new, want)]:
            actual = board(reader, output, rows, language)
            assert all(matches(a, b) for a, b in zip(actual, expected)), (
                book.name,
                tag,
                reader,
            )
    assert initial == digest(book)
    return {
        "sha256": initial,
        "locale": language,
        "formulas": len(rows),
        "baselineDifferences": wrong,
        "candidateDifferences": 0,
        "candidateSavedTypedCaches": len(rows),
        "readerWriterPairs": 4,
    }


def binary(old, new, book, out):
    values = [
        0,
        1,
        -1,
        17,
        -17,
        536870911,
        -536870912,
        536870912,
        -536870913,
        3412312,
        0.01,
        -0.01,
        12345.67,
        -12345.67,
        0.5,
        -0.5,
        1.23456789,
        -1.23456789,
        1.5e22,
        2**-41,
    ]
    special = [
        "#DIV/0!",
        "#N/A",
        "#NUM!",
        "#REF!",
        "#VALUE!",
        "#NULL!",
        "#NAME?",
        "TRUE",
        "FALSE",
        "中文",
    ]
    expected = [
        [
            str(n),
            str(n),
            str((n > 0) - (n < 0)),
            "",
            special[i] if i < len(special) else "",
        ]
        for i, n in enumerate(values)
    ]

    def check(cli, path):
        actual = csv_range(cli, path, "B1:F20", "en")
        assert len(actual) == len(expected)
        for row, answer in zip(actual, expected):
            assert len(row) == len(answer) and all(
                matches(a, b)
                or (i < 3 and "E" in a and float(a) == float(f"{float(b):.5E}"))
                for i, (a, b) in enumerate(zip(row, answer))
            ), (path, row, answer)

    initial = digest(book)
    for tag, writer in [("old", old), ("new", new)]:
        check(writer, book)
        output = out / f"{book.stem}-{book.suffix[1:]}-{tag}.xlsx"
        save(writer, book, output, "en")
        with ZipFile(output) as z:
            sheet = ET.fromstring(z.read("xl/worksheets/sheet1.xml"))
            formulas = list(sheet.iterfind(".//{*}f"))
            assert (book.suffix == ".xls") == bool(formulas)
            for i, value in enumerate(values, 1):
                for col, wanted in (
                    ("B", value),
                    ("C", value),
                    ("D", (value > 0) - (value < 0)),
                ):
                    cell = sheet.find(f".//{{*}}c[@r='{col}{i}']")
                    assert cell is not None and cell.get("t", "n") == "n"
                    assert matches(cell.findtext("{*}v"), str(wanted))
            for i, val in enumerate(special, 1):
                cell = sheet.find(f".//{{*}}c[@r='F{i}']")
                assert cell is not None
                assert cell.get("t") in (
                    {"e"}
                    if val.startswith("#")
                    else {"b"}
                    if val in ("TRUE", "FALSE")
                    else {"s", "inlineStr", "str"}
                )
        for reader in [old, new]:
            check(reader, output)
    assert digest(book) == initial
    return {
        "sha256": initial,
        "checkedCells": 70,
        "readerWriterPairs": 4,
        "importedFormulas": len(formulas),
    }


def native(program, year, out):
    original = DATA / f"calendar-{year}.xlsx"
    edited = DATA / f"edited-{year}.xlsx"
    original_hash = digest(original)
    states = [[v for _, _, v in cells(book)] for book in (original, edited)]
    changed = [i for i, (a, b) in enumerate(zip(*states)) if a != b]
    assert changed
    indices = sorted(set(changed + list(range(0, len(states[0]), 17))))

    def launch(name):
        number = port()
        profile = out / f"profile-{year}-{name}"
        return SheetControl(number, profile), app(
            program,
            ["--control", str(number), "--diagnostics"],
            out,
            {"LC_ALL": REGIONS["zh"], "JOYSHEET_CONFIG_DIR": str(profile)},
            f"window-{year}-{name}.log",
            unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        )

    def inspect(call, state):
        call("ui.awaitIdle")
        for i in indices:
            got = call("cell.get", cell=f"G{i + 2}")["text"]
            assert matches(got, states[state][i]), (year, i, got, states[state][i])

    output = out / f"工作日重算-{year}.xlsx"
    client, session = launch("edit")
    with session as process:
        client.wait(lambda s: s["perf"]["frame"] > 1, "native workbook", process)
        call = client.call
        call("app.open", path=str(original))
        inspect(call, 0)
        call("engine.execute", command="edit.goTo", params={"reference": "B1"})
        call("ui.cell.click", cell="B1")
        call("ui.edit.text", text="45557")
        call("ui.edit.commit")
        inspect(call, 1)
        for cmd, expected in [("edit.undo", 0), ("edit.redo", 1)]:
            call("engine.execute", command=cmd, params={})
            inspect(call, expected)
        call("app.save", path=str(output))
    saved = cells(output)
    assert saved == cells(edited)
    client, session = launch("reopen")
    with session as process:
        client.wait(lambda s: s["perf"]["frame"] > 1, "reopened workbook", process)
        client.call("app.open", path=str(output))
        inspect(client.call, 1)
    assert digest(original) == original_hash
    return {
        "changedFormulaResults": len(changed),
        "cellsPerState": len(indices),
        "states": 5,
        "savedTypedCaches": len(saved),
        "undoRedo": True,
        "freshProcessReopen": True,
        "originalUnchanged": True,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["baseline", "candidate", "output"]:
        parser.add_argument(name, type=Path)
    parser.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    old, new, out = (p.resolve() for p in (args.baseline, args.candidate, args.output))
    out.mkdir(parents=True, exist_ok=True)
    result = {
        "binaries": [digest(old), digest(new)],
        "calendars": {
            p.name: calendar(old, new, p, out) for p in sorted(DATA.glob("*.xlsx"))
        },
        "binaryReaders": {
            p.name: binary(old, new, p, out)
            for p in sorted(DATA.glob("*.xls*"))
            if p.suffix in {".xls", ".xlsb"}
        },
    }
    if args.desktop:
        result["native"] = {
            str(yr): native(args.desktop.resolve(), yr, out) for yr in (1900, 1904)
        }
    (out / "result.json").write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    )


if __name__ == "__main__":
    main()
