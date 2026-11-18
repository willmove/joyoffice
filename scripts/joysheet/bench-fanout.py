"""Measure a real GPUI commit through the first matching CPU paint on an isolated workbook."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import runpy
import socket
import subprocess
import xml.etree.ElementTree as ET
import zipfile

# Reuse the authenticated transport, startup and idle checks of the native smoke suite.
Control = runpy.run_path(str(Path(__file__).with_name("smoke-native.py")))["Control"]


def run(binary, rows, output):
    output.mkdir(parents=True, exist_ok=False)
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    profile = output / "profile"
    env = dict(os.environ, JOYSHEET_CONFIG_DIR=str(profile))
    for name in ("JOYSHEET_CONTROL_TOKEN", "GRIDCRAFT_CONTROL_TOKEN"):
        env.pop(name, None)
    client = Control(port, profile)
    call = client.call
    with (output / "desktop.log").open("w") as log:
        proc = subprocess.Popen([binary, "--control", str(port), "--diagnostics"], env=env,
                                stdout=log, stderr=subprocess.STDOUT)
        try:
            client.wait(lambda v: v["perf"]["frame"] > 1, "first frame", proc)
            def execute(command, **params):
                call("engine.execute", command=command, params=params)
                call("ui.awaitIdle")

            # Separate setup from timed edits. Keep each control request comfortably within budget.
            for start in range(1, rows + 1, 5000):
                chunk = [[f"=$A$1+{i}"] for i in range(start, min(rows + 1, start + 5000))]
                execute("range.setValues", range=f"B{start}", values=chunk)
            execute("cell.set", cell="D1", input=f"=SUM(B1:B{rows})")
            execute("edit.goTo", reference="A1")
            def verify(edit):
                tail = call("cell.get", cell=f"B{rows}")
                total = call("cell.get", cell="D1")
                if (tail["value"], tail["formula"], total["value"], total["formula"]) != (
                    rows + edit, f"=$A$1+{rows}", rows * (rows + 1) // 2 + rows * edit, f"=SUM(B1:B{rows})"
                ):
                    raise AssertionError((edit, tail, total))

            for edit in range(1, 23):
                if edit == 3:
                    call("ui.perf", reset=True)
                call("ui.cell.click", cell="A1")
                call("ui.edit.text", text=str(edit))
                call("ui.edit.commit")
                call("ui.awaitIdle")
                verify(edit)
            stages = call("ui.perf", raw=True)["stages"]
            if (stages["editCount"], stages["pending"], stages["discarded"]) != (20, 0, 0):
                raise AssertionError("missing or stale CPU paint measurements")
            if any(edit["formulasEvaluated"] != rows + 1 for edit in stages["edits"]):
                raise AssertionError("incorrect number of evaluated formulas")
            execute("edit.undo")
            verify(21)
            execute("edit.redo")
            verify(22)
            workbook = output / "重算 副本.xlsx"
            call("app.save", path=str(workbook))
            with zipfile.ZipFile(workbook) as archive:
                xml = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
            ns = {"s": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}
            total_cell = xml.find(".//s:c[@r='D1']", ns)
            if (total_cell is None or total_cell.findtext("s:f", namespaces=ns) != f"SUM(B1:B{rows})"
                    or float(total_cell.findtext("s:v", namespaces=ns)) != rows * (rows + 1) // 2 + rows * 22):
                raise AssertionError("independent XLSX formula/value mismatch")
            call("app.open", path=str(workbook))
            verify(22)
            system = f"{platform.system()} {platform.release()} {platform.machine()}"
            report = {"system": system, "rows": rows, "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                      "scope": stages["scope"], "undoRedoAndXlsxVerified": True, "edits": stages["edits"]}
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=8)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
    if platform.system() in ("Darwin", "Linux"):
        import resource
        # This CLI starts exactly one child. Measure its lifetime through save and reopen.
        peak = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        report["desktopPeakRssBytes"] = peak if platform.system() == "Darwin" else peak * 1024
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--rows", choices=(100, 10000, 100000), type=int, default=100000)
    args = parser.parse_args()
    run(args.binary.resolve(), args.rows, args.output.resolve())
