"""Exercise six lookup functions through an editable desktop workbook with duplicate keys."""

import argparse
import json
import math
import subprocess
import time
from pathlib import Path
import sys
import xml.etree.ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port


def run(binary, out):
    number = port()
    profile = out / "profile"
    client = SheetControl(number, profile)
    call = client.call
    rows = 4096
    formulas = ["=MATCH(F1,A1:A4096,0)", "=VLOOKUP(F1,A1:B4096,2,FALSE)",
                "=HLOOKUP(F1,H1:J2,2,FALSE)", "=LOOKUP(F1,A1:A4096,B1:B4096)",
                "=XLOOKUP(F1,A1:A4096,B1:B4096,0,0,-1)", "=XMATCH(F1+0.5,A1:A4096,-1,-1)",
                "=XMATCH(F1+0.5,A1:A4096,1,1)"]
    def execute(command, **params):
        call("engine.execute", command=command, params=params)
        call("ui.awaitIdle")

    def verify(key):
        expected = [2*key-1, 3*(2*key-1), {321: 12, 1229: 18}[key], 6*key, 6*key, 2*key, 2*key+1]
        for i, (value, formula) in enumerate(zip(expected, formulas), 1):
            cell = call("cell.get", cell=f"D{i}")
            assert (cell["value"], cell["formula"]) == (value, formula), (i, key, cell)
        return expected

    with app(binary, ["--control", str(number), "--diagnostics"], out,
             {"JOYSHEET_CONFIG_DIR": str(profile)}, unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN")) as process:
        client.wait(lambda v: v["perf"]["frame"] > 1, "first desktop frame", process)
        for start in range(1, rows + 1, 1024):
            execute("range.setValues", range=f"A{start}", values=[
                [str((i + 1)//2), str(i*3)] for i in range(start, start+1024)])
        execute("range.setValues", range="H1", values=[["321","1229","1500"],["12","18","29"]])
        execute("cell.set", cell="F1", input="321")
        execute("range.setValues", range="D1", values=[[f] for f in formulas])
        initial = verify(321)
        execute("edit.goTo", reference="F1")
        call("ui.cell.click", cell="F1")
        call("ui.edit.text", text="1229")
        call("ui.edit.commit")
        call("ui.awaitIdle")
        edited = verify(1229)
        execute("edit.undo")
        verify(321)
        execute("edit.redo")
        verify(1229)
        saved = out / "查找 重开.xlsx"
        call("app.save", path=str(saved))
        with ZipFile(saved) as archive:
            sheet = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
        for i, (formula, value) in enumerate(zip(formulas, edited), 1):
            cell = sheet.find(f".//{{*}}c[@r='D{i}']")
            assert cell.findtext("{*}f").removeprefix("_xlfn.") == formula[1:]
            assert float(cell.findtext("{*}v")) == value
        call("app.open", path=str(saved))
        call("ui.awaitIdle")
        verify(1229)
    return {"binarySha256": digest(binary), "rows": rows, "formulas": formulas,
            "beforeEdit": initial, "afterEdit": edited, "undoRedoAndXlsxReopen": True}


def compare_cli(versions, workbook, expected, rounds):
    """Time opening the saved GUI workbook and committing twelve changes through the CLI."""
    edits = [part for key in [321, 1229] * 6 for part in
             ("--cmd", "cell.set=" + json.dumps({"cell": "F1", "input": str(key)}))]
    def sample(cli):
        start = time.perf_counter_ns()
        result = subprocess.run([cli, "run", "--in", workbook, *edits, "--quiet",
                                 "--print", "D1:D7", "--csv"], capture_output=True, check=True)
        elapsed = (time.perf_counter_ns() - start) / 1_000_000
        assert list(map(int, result.stdout.decode().splitlines())) == expected
        return round(elapsed, 4)
    timings = [[], []]
    for cli in versions:
        sample(cli)
    for group in range(rounds):
        order = (0, 1, 1, 0) if group % 2 == 0 else (1, 0, 0, 1)
        for _ in range(10):
            for i in order:
                timings[i].append(sample(versions[i]))
    return {label: {"cliSha256": digest(cli), "samplesMs": t, "p50Ms": sorted(t)[math.ceil(0.50 * len(t))-1],
                    "p95Ms": sorted(t)[math.ceil(0.95 * len(t))-1]} for label, cli, t in zip(("before", "after"), versions, timings)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("binary", "output"):
        parser.add_argument(key, type=Path)
    parser.add_argument("--compare-cli", nargs=2, type=Path, metavar=("BASELINE", "CANDIDATE"))
    parser.add_argument("--timing-rounds", type=int, default=1, help="20 samples per build per round; the starting order alternates")
    args = parser.parse_args()
    if args.timing_rounds < 1:
        parser.error("--timing-rounds must be positive")
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    report = run(args.binary.resolve(), out)
    if args.compare_cli:
        report["openAndTwelveLookupEdits"] = compare_cli([cli.resolve() for cli in args.compare_cli],
                                                         out / "查找 重开.xlsx", report["afterEdit"], args.timing_rounds)
    (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
