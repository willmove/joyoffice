#!/usr/bin/env python3
"""Run the local M0-M7 headless validation suite and retain per-command evidence.

No GUI, Office, reader, physical printing, distribution signature or hardware
acceptance is inferred. Each command has its own timeout and raw log. A failed
step does not hide later independent failures; the final exit remains nonzero.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

from m7_evidence import ROOT, digest, environment, source_identity


def steps() -> list[tuple[str, list[str]]]:
    return [
        ("docs", [sys.executable, "scripts/check_docs.py"]),
        ("acceptance", [sys.executable, "scripts/check_acceptance.py"]),
        ("gates", [sys.executable, "scripts/check_gates.py"]),
        ("python-tests", [sys.executable, "-m", "unittest", "discover", "-s", "scripts/tests"]),
        ("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
        ("core-tests", ["cargo", "test", "--locked"]),
        ("app-tests", ["cargo", "test", "--locked", "-p", "joyword"]),
        ("ai-enabled-tests", ["cargo", "test", "--locked", "-p", "joyword-shell", "--features", "ai"]),
        ("ai-disabled-check", ["cargo", "check", "--locked", "--no-default-features", "-p", "joyword-engine", "-p", "joyword-model", "-p", "joyword-docx", "-p", "joyword-layout", "-p", "joyword-shell", "-p", "joyword-agent"]),
        ("clippy", ["cargo", "clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"]),
        ("clippy-ai", ["cargo", "clippy", "-p", "joyword-shell", "--all-targets", "--features", "ai", "--locked", "--", "-D", "warnings"]),
        ("recovery-tests", ["cargo", "test", "--manifest-path", "experiments/recovery-journal/Cargo.toml", "--locked"]),
        ("robustness-tests", ["cargo", "test", "--manifest-path", "experiments/docx-robustness/Cargo.toml", "--locked"]),
        ("robustness-driver-tests", [sys.executable, "-m", "unittest", "discover", "-s", "experiments/docx-robustness/tests", "-p", "test_*.py"]),
        ("layout-corpus-smoke", ["cargo", "run", "--locked", "-p", "joyword-layout", "--example", "m7_performance", "--", "10", "1", "2"]),
        ("release-evidence-audit", [sys.executable, "scripts/check_release.py", "--audit"]),
        ("native-build", ["cargo", "build", "--locked", "-p", "joyword", "-p", "joyword-cli"]),
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=1800)
    args = parser.parse_args()
    if not 0 < args.timeout <= 7200:
        parser.error("timeout must be in (0, 7200]")
    args.out.mkdir(parents=True, exist_ok=True)
    report = {"schema": 1, "source": source_identity(), "environment": environment(),
              "scope": "local-headless-and-native-build-only", "status": "incomplete",
              "unverified": ["GUI/IME/DPI", "Office exchange", "screen readers", "printing",
                             "other OS native sessions", "signed installation/upgrade", "Xinchuang hardware"],
              "steps": []}
    for name, invocation in steps():
        path = args.out / f"{name}.log"
        start = time.monotonic()
        error = None
        with path.open("wb") as log:
            try:
                result = subprocess.run(invocation, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, timeout=args.timeout, check=False)
                code = result.returncode
            except (OSError, subprocess.TimeoutExpired) as exception:
                code, error = -1, str(exception)
        report["steps"].append({"name": name, "command": invocation, "exit_code": code,
                                "seconds": time.monotonic() - start, "log": path.name,
                                "sha256": digest(path), "error": error})
        (args.out / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(f"{name}: {'PASS' if code == 0 else 'FAIL'} ({report['steps'][-1]['seconds']:.2f}s)", flush=True)
    success = all(step["exit_code"] == 0 for step in report["steps"])
    report["status"] = "passed" if success else "failed"
    (args.out / "summary.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0 if success else 1


if __name__ == "__main__":
    raise SystemExit(main())
