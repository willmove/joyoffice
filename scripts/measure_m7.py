#!/usr/bin/env python3
"""Measure the release model/layout benchmark, retaining raw timings and memory.

These are warm CPU layout timings, NOT GUI startup, scrolling or input-to-paint.
The default is a quick smoke. Use --samples 30 --events 1000 for full cohorts.
"""
from __future__ import annotations

import argparse
import ctypes
import json
import os
import subprocess
import time
from collections import defaultdict
from pathlib import Path

from m7_evidence import ROOT, digest, distribution, environment, source_identity


def windows_memory(process) -> dict | None:
    if os.name != "nt":
        return None
    from ctypes import wintypes

    class Counters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
            (name, ctypes.c_size_t) for name in (
                "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage",
                "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage",
                "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")]

    counters = Counters()
    counters.cb = ctypes.sizeof(counters)
    api = ctypes.WinDLL("psapi", use_last_error=True).GetProcessMemoryInfo
    api.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    api.restype = wintypes.BOOL
    # Popen owns this child-process handle and keeps it alive until exit.
    if not api(int(process._handle), ctypes.byref(counters), counters.cb):
        return None
    return {"rss_bytes": counters.WorkingSetSize, "peak_rss_bytes": counters.PeakWorkingSetSize}


def power_state() -> dict:
    if os.name != "nt":
        return {"status": "unobserved"}
    class Power(ctypes.Structure):
        _fields_ = [(name, ctypes.c_ubyte) for name in ("ac", "flag", "percent", "reserved")] + [
            ("life", ctypes.c_uint32), ("full", ctypes.c_uint32)]
    power = Power()
    if not ctypes.windll.kernel32.GetSystemPowerStatus(ctypes.byref(power)):
        return {"status": "unobserved"}
    return {"ac_line_status": power.ac, "battery_percent": power.percent}


def measure(binary: Path, pages: int, samples: int, events: int, out: Path, timeout: float) -> dict:
    raw = out / f"{pages}-pages.jsonl"
    errors = out / f"{pages}-pages.stderr"
    memory = []
    start = time.monotonic()
    with raw.open("wb") as stdout, errors.open("wb") as stderr:
        with subprocess.Popen([str(binary), str(pages), str(samples), str(events)], cwd=ROOT,
                              stdout=stdout, stderr=stderr) as process:
            while process.poll() is None:
                sample = windows_memory(process)
                if sample is not None:
                    memory.append({"seconds": time.monotonic() - start, **sample})
                if time.monotonic() - start > timeout:
                    process.kill()
                    process.wait()
                    raise RuntimeError(f"{pages}-page benchmark timed out; raw evidence retained")
                time.sleep(.05)
            if process.returncode:
                raise RuntimeError(f"{pages}-page benchmark failed: {errors.read_text(encoding='utf-8')}")
    records = [json.loads(line) for line in raw.read_text(encoding="utf-8").splitlines()]
    expected = {"full_layout": samples, "first_edit_layout": events, "last_edit_layout": events,
                "pre_cancel": 1, "identity": 1, "complete": 1}
    counts = defaultdict(int)
    timings = defaultdict(list)
    for record in records:
        counts[record["kind"]] += 1
        if "ms" in record:
            timings[record["kind"]].append(record["ms"])
    if dict(counts) != expected or records[0]["pages"] != pages or records[-1].get("scene_equivalence_checks") != events * 2:
        raise RuntimeError(f"incomplete {pages}-page sample matrix: {dict(counts)}")
    memory_file = out / f"{pages}-pages-memory.json"
    memory_file.write_text(json.dumps(memory, indent=2) + "\n", encoding="utf-8")
    return {"pages": pages, "identity": records[0], "timings_ms": {
        kind: distribution(values) for kind, values in timings.items()},
        "raw": raw.name, "raw_sha256": digest(raw),
        "memory": {"status": "observed" if memory else "unobserved", "raw": memory_file.name,
                   "raw_sha256": digest(memory_file),
                   "peak_rss_bytes": max((item["peak_rss_bytes"] for item in memory), default=None),
                   "stable_rss_bytes": None,
                   "note": "50 ms process working-set samples; no stable-idle RSS or GPU measurement"}}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/examples" / ("m7_performance.exe" if os.name == "nt" else "m7_performance"))
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--pages", nargs="+", type=int, choices=[10, 100, 1000], default=[10, 100, 1000])
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--events", type=int, default=10)
    parser.add_argument("--timeout", type=float, default=1800)
    parser.add_argument("--hardware-profile", required=True)
    args = parser.parse_args()
    if not 1 <= args.samples <= 100 or not 2 <= args.events <= 2000 or not 0 < args.timeout <= 7200:
        parser.error("sample, event or timeout budget exceeded")
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error("build first: cargo build --locked --release -p joyword-layout --example m7_performance")
    args.out.mkdir(parents=True, exist_ok=True)
    report = {"schema": 1, "scope": "warm-model-layout-only", "source": source_identity(),
              "environment": environment(), "hardware_profile": args.hardware_profile,
              "power": power_state(), "binary_sha256": digest(binary),
              "required_sample_counts_met": args.samples >= 30 and args.events >= 1000,
              "gui_budgets_verified": False, "status": "incomplete", "tiers": []}
    try:
        for pages in args.pages:
            tier = measure(binary, pages, args.samples, args.events, args.out, args.timeout)
            report["tiers"].append(tier)
            print(json.dumps({"pages": pages, "timings_ms": tier["timings_ms"]}), flush=True)
        report["status"] = "complete"
    finally:
        (args.out / "summary.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
