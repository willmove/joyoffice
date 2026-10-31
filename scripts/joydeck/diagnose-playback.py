"""Temporary diagnostic branch only: sample a hung, owned playback test."""
import json
import os
from pathlib import Path
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "artifacts/playback-diagnostic"
OUT.mkdir(parents=True, exist_ok=True)
artifacts = [json.loads(line) for line in (OUT / "build.json").read_text(encoding="utf-8").splitlines()]
executables = [row["executable"] for row in artifacts if row.get("reason") == "compiler-artifact"
               and row.get("executable") and row["target"]["name"] == "playback"]
assert len(executables) == 1, executables
environment = dict(os.environ, RUST_BACKTRACE="full")
started = time.monotonic()
report = {"test": "h264_real_decode_pause_twenty_seeks_and_bounded_frame", "timedOut": False}
with (OUT / "test.log").open("w", encoding="utf-8") as log:
    child = subprocess.Popen([executables[0], "--exact", report["test"], "--nocapture"],
                             cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT,
                             start_new_session=True)
    report["testPid"] = child.pid
    try:
        report["returncode"] = child.wait(timeout=45)
    except subprocess.TimeoutExpired:
        report["timedOut"] = True
        lines = subprocess.check_output(["ps", "-axo", "pid=,ppid=,comm="], text=True).splitlines()
        processes = []
        for line in lines:
            fields = line.strip().split(None, 2)
            if len(fields) == 3:
                processes.append({"pid": int(fields[0]), "parent": int(fields[1]), "executable": fields[2]})
        owned = {child.pid}
        while True:
            expanded = owned | {row["pid"] for row in processes if row["parent"] in owned}
            if expanded == owned:
                break
            owned = expanded
        report["ownedProcesses"] = [row for row in processes if row["pid"] in owned]
        for row in report["ownedProcesses"]:
            pid = str(row["pid"])
            with (OUT / ("sample-" + pid + "-command.log")).open("w", encoding="utf-8") as output:
                try:
                    subprocess.run(["sample", pid, "3", "-file", str(OUT / ("sample-" + pid + ".txt"))],
                                   stdout=output, stderr=subprocess.STDOUT, timeout=15, check=False)
                except subprocess.TimeoutExpired:
                    output.write("Sampling timed out.\n")
            with (OUT / ("files-" + pid + ".txt")).open("w", encoding="utf-8") as output:
                try:
                    subprocess.run(["lsof", "-p", pid, "-Fpn"], stdout=output, stderr=subprocess.STDOUT,
                                   timeout=5, check=False)
                except subprocess.TimeoutExpired:
                    output.write("File inventory timed out.\n")
        if child.poll() is None and os.getpgid(child.pid) == child.pid:
            os.killpg(child.pid, signal.SIGKILL)
        try:
            report["returncode"] = child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            report["killDidNotReapWithinEightSeconds"] = True
report["seconds"] = time.monotonic() - started
(OUT / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, indent=2), flush=True)
print((OUT / "test.log").read_text(encoding="utf-8"), flush=True)
raise SystemExit(1 if report["timedOut"] else report["returncode"])
