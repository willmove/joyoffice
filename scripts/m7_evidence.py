"""Shared, standard-library evidence utilities for M7 tooling."""
from __future__ import annotations

import hashlib
import math
import platform
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def command(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True, encoding="utf-8").strip()


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_identity() -> dict:
    diff = subprocess.check_output(["git", "diff", "HEAD", "--binary"], cwd=ROOT)
    untracked = command("git", "ls-files", "--others", "--exclude-standard").splitlines()
    return {
        "head": command("git", "rev-parse", "HEAD"),
        "tracked_diff_sha256": hashlib.sha256(diff).hexdigest(),
        "untracked": {name: digest(ROOT / name) for name in untracked if (ROOT / name).is_file()},
        "dirty": bool(diff or untracked),
    }


def environment() -> dict:
    return {"system": platform.platform(), "machine": platform.machine(),
            "processor": platform.processor(), "python": platform.python_version(),
            "rustc": command("rustc", "--version")}


def distribution(samples: list[float]) -> dict:
    if not samples or any(not math.isfinite(item) or item < 0 for item in samples):
        raise ValueError("timing samples must be finite, nonnegative, and nonempty")
    ordered = sorted(samples)
    return {"count": len(samples), "method": "nearest-rank", **{
        name: ordered[max(0, math.ceil(len(samples) * quantile) - 1)]
        for name, quantile in [("p50", .50), ("p95", .95), ("p99", .99), ("max", 1.0)]}}
