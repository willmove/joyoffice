"""Verify INDIRECT and OFFSET against Chinese and English Excel caches and real Sheet sessions."""

import argparse
import csv
import io
import json
import os
import runpy
import statistics
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

cached_values = runpy.run_path(str(Path(__file__).with_name("check-broadcast.py")))[
    "cached_values"
]
FIXTURES = Path(__file__).parent / "tests/data/dynamic-references"
LOCALES = {"en": "en_US.UTF-8", "zh": "zh_CN.UTF-8"}


def read_board(cli, book, locale, nrows):
    command = [cli, "cat", book, "--range", f"G1:G{nrows}", "--sheet", "Probe", "--csv"]
    answer = subprocess.run(
        command,
        env={**os.environ, "LC_ALL": locale},
        capture_output=True,
        check=True,
        timeout=120,
    )
    lines = list(csv.reader(io.StringIO(answer.stdout.decode())))
    assert len(lines) == nrows and all(len(row) == 1 for row in lines)
    return [r[0] for r in lines]


def expected(book):
    cached = cached_values(book)
    formulas = formula_text(book)
    values = []
    for i in range(len(formulas)):
        value = cached["G" + str(i + 1)]
        values.append(
            str(int(value))
            if isinstance(value, float) and value.is_integer()
            else str(value)
        )
    return values


def formula_text(book):
    with ZipFile(book) as z:
        sheet = ET.fromstring(z.read("xl/worksheets/sheet1.xml"))
        texts = {}
        for cell in sheet.iterfind(".//{*}c"):
            node = cell.find("{*}f")
            addr = cell.get("r")
            if addr.startswith("G") and addr[1:].isdecimal() and node is not None:
                texts[addr] = node.text
        return texts


def cli_exchange(old, new, book, out, locale, want):
    original = digest(book)
    count = len(want)
    previous = read_board(old, book, locale, count)
    actual = read_board(new, book, locale, count)
    assert actual == want, [
        (i + 1, a, b) for i, (a, b) in enumerate(zip(actual, want)) if a != b
    ]

    def differences(values):
        return [f"G{i + 1}" for i, (x, y) in enumerate(zip(values, want)) if x != y]

    diffs = differences(previous)
    assert diffs
    original_text = formula_text(book)
    for program, reader, anticipated, label in (
        (old, new, want, "older"),
        (new, old, previous, "newer"),
    ):
        dest = out / f"{book.stem}-{label}.xlsx"
        subprocess.run(
            [
                program,
                "run",
                "--in",
                book,
                "--cmd",
                'cell.set={"cell":"L1","input":"exchange"}',
                "--out",
                dest,
                "--quiet",
            ],
            env={**os.environ, "LC_ALL": locale},
            capture_output=True,
            check=True,
            timeout=120,
        )
        assert formula_text(dest) == original_text
        assert read_board(reader, dest, locale, count) == anticipated
    assert digest(book) == original
    return {
        "sourceSha256": original,
        "independentCells": count,
        "previousDifferences": diffs,
        "differences": 0,
        "bothGenerationsReopened": True,
        "originalFormulasRetained": len(original_text),
    }


def desktop(program, book, out, locale, want):
    def launch(tag):
        number = port()
        profile = out / (tag + "-profile")
        return SheetControl(number, profile), app(
            program,
            ["--control", str(number), "--diagnostics"],
            out,
            {"LC_ALL": locale, "JOYSHEET_CONFIG_DIR": str(profile)},
            tag + ".log",
            unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        )

    def verify(call, values):
        for addr, value in values.items():
            actual = call("cell.get", cell=addr)["text"]
            assert actual == value, (addr, actual, value)

    def change(call, address, value):
        call("engine.execute", command="edit.goTo", params={"reference": address})
        call("ui.cell.click", cell=address)
        call("ui.edit.text", text=str(value))
        call("ui.edit.commit")
        call("ui.awaitIdle")

    def action(call, command):
        call("engine.execute", command=command, params={})
        call("ui.awaitIdle")

    selected = {
        f"G{i}": want[i - 1]
        for i in [
            2,
            6,
            17,
            20,
            54,
            66,
            77,
            78,
            81,
            92,
            93,
            98,
            100,
            101,
            106,
            112,
            119,
            120,
            123,
            125,
            130,
        ]
    }
    old_sha = digest(book)
    saved = out / "动态引用 编辑.xlsx"
    client, process = launch("editor")
    with process as p:
        client.wait(lambda v: v["perf"]["frame"] > 1, "native editor frame", p)
        call = client.call
        call("app.open", path=str(book))
        call("ui.awaitIdle")
        verify(call, selected)
        # B1 chooses which row G92 reads. The inactive target must not supply G92's value.
        change(call, "B1", 19)
        new = {**selected, "G92": "192"}
        verify(call, new)
        change(call, "B19", 881)
        new = {**new, "G92": "881"}
        verify(call, new)
        action(call, "edit.undo")
        verify(call, {**new, "G92": "192"})
        action(call, "edit.redo")
        verify(call, new)
        call("app.save", path=str(saved))
    assert formula_text(saved) == formula_text(book)
    client, process = launch("reopen")
    with process as p:
        client.wait(lambda v: v["perf"]["frame"] > 1, "fresh editor frame", p)
        client.call("app.open", path=str(saved))
        client.call("ui.awaitIdle")
        verify(client.call, new)
    assert digest(book) == old_sha
    return {
        "binarySha256": digest(program),
        "cellsPerState": len(selected),
        "states": 6,
        "targetRow": [12, 19],
        "targetValue": [122, 192, 881],
        "undoRedo": True,
        "savedAndReopened": True,
        "originalUnchanged": True,
    }


def benchmark(old, new, out):
    rows = 8192
    report = {}
    environment = {**os.environ, "LC_ALL": LOCALES["en"]}
    for mode, ref in (("a1", 'INDIRECT("$B$1")'), ("r1c1", 'INDIRECT("R1C2",FALSE)')):
        source = out / (mode + ".xlsx")
        script = out / (mode + ".jsonl")
        commands = [
            {"command": "cell.set", "params": {"cell": "B1", "input": "7"}},
            {
                "command": "range.setValues",
                "params": {
                    "range": "A2",
                    "values": [[f"={ref}+{i}"] for i in range(2, rows + 2)],
                },
            },
        ]
        script.write_text("".join(json.dumps(cmd) + "\n" for cmd in commands))
        subprocess.run(
            [old, "run", "--script", script, "--out", source, "--quiet"],
            capture_output=True,
            check=True,
            timeout=180,
        )
        samples = {"baseline": [], "candidate": []}
        versions = [("baseline", old), ("candidate", new)]
        for i in range(21):
            for label, binary in versions if i % 2 == 0 else reversed(versions):
                start = time.perf_counter_ns()
                result = subprocess.run(
                    [
                        binary,
                        "run",
                        "--in",
                        source,
                        "--cmd",
                        'cell.set={"cell":"B1","input":"12"}',
                        "--print",
                        f"A{rows + 1}",
                        "--csv",
                        "--quiet",
                    ],
                    env=environment,
                    capture_output=True,
                    check=True,
                    timeout=120,
                )
                duration = (time.perf_counter_ns() - start) / 1e6
                assert result.stdout.decode().strip() == str(rows + 13)
                if i:
                    samples[label].append(duration)
        report[mode] = {
            "rows": rows,
            "rounds": 20,
            **{
                label: {"p50": statistics.median(values), "p95": sorted(values)[18]}
                for label, values in samples.items()
            },
        }
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--desktop", type=Path)
    parser.add_argument("--benchmark-only", action="store_true")
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True)
    old, new = args.baseline.resolve(), args.candidate.resolve()
    if args.benchmark_only:
        timings = benchmark(old, new, out)
        (out / "timing.json").write_text(json.dumps(timings, indent=2) + "\n")
        return
    data = {"cliSha256": [digest(old), digest(new)], "exchange": {}, "desktop": {}}
    for lang, locale in LOCALES.items():
        for offset in (0, 7):
            book = (FIXTURES / f"{lang}-{offset}.xlsx").resolve()
            want = expected(book)
            data["exchange"][book.name] = cli_exchange(
                old, new, book, out, locale, want
            )
            if args.desktop and offset == 0:
                dest = out / lang
                dest.mkdir()
                data["desktop"][lang] = desktop(
                    args.desktop.resolve(), book, dest, locale, want
                )
    (out / "report.json").write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
