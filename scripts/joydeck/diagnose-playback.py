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
def run_case(label, extra_args):
    case = OUT / label
    case.mkdir()
    environment = dict(os.environ)
    started = time.monotonic()
    report = {"selection": extra_args, "timedOut": False}
    with (case / "test.log").open("w", encoding="utf-8") as log:
        command = ["cargo", "test", "-p", "joydeck", "-p", "joydeck-cli", "-p", "joydeck-engine", "-p", "joydeck-export", "-p", "joydeck-formats", "-p", "joydeck-model", "-p", "joydeck-platform", "-p", "joydeck-scene", "-p", "joydeck-shell", "-p", "joyoffice-opc", "--locked", *extra_args]
        if os.environ.get("DIAGNOSTIC_MODE") == "entrypoint":
            command = ["cargo", "xtask", "check", "joydeck"]
        child = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT,
                                 start_new_session=True)
        report["testPid"] = child.pid
        try:
            deadline = started + 420
            while child.poll() is None:
                contents = (case / "test.log").read_text(encoding="utf-8", errors="replace")
                if "Running tests/playback.rs" in contents:
                    deadline = min(deadline, time.monotonic() + 45) if not report.get("playbackStarted") else deadline
                    report["playbackStarted"] = True
                if time.monotonic() >= deadline:
                    raise subprocess.TimeoutExpired(child.args, time.monotonic() - started)
                time.sleep(0.2)
            report["returncode"] = child.returncode
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
                with (case / ("sample-" + pid + "-command.log")).open("w", encoding="utf-8") as output:
                    try:
                        subprocess.run(["sample", pid, "3", "-file", str(case / ("sample-" + pid + ".txt"))],
                                       stdout=output, stderr=subprocess.STDOUT, timeout=15, check=False)
                    except subprocess.TimeoutExpired:
                        output.write("Sampling timed out.\n")
                with (case / ("files-" + pid + ".txt")).open("w", encoding="utf-8") as output:
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
    (case / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2), flush=True)
    print((case / "test.log").read_text(encoding="utf-8"), flush=True)
    return report

mode = os.environ.get("DIAGNOSTIC_MODE", "parallel")
arguments = ["--", "--test-threads=1"] if mode == "serial" else []
reports = []
for index in range(1):
    report = run_case(f"{mode}-{index}", arguments)
    reports.append(report)
    if report["timedOut"] or report["returncode"] != 0:
        break
(OUT / "summary.json").write_text(json.dumps({"mode": mode, "reports": reports}, indent=2) + "\n", encoding="utf-8")
raise SystemExit(1 if any(row["timedOut"] or row["returncode"] != 0 for row in reports) else 0)
