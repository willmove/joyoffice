#!/usr/bin/env python3
"""Fail closed on the M0-M7 release gates; --audit reports pending work without releasing.

Evidence marked passed must be a repository-local file with a matching SHA-256.
Xinchuang hardware observations remain explicit nonblocking follow-ups per the
2026-10-18 decision. They are never converted into passed observations.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from m7_evidence import ROOT, digest

TASK = re.compile(r"^### (T\d{3})：([^\n]+)\n(.*?)(?=^### |^## |\Z)", re.M | re.S)
STATE = re.compile(r"^- \[([ x])\] 状态：`(planned|in_progress|blocked|done)`$", re.M)
REQUIRED = {
    "windows-native", "macos-native", "linux-x11-native", "linux-wayland-native",
    "accessibility-readers", "ime-dpi", "performance-gui", "recovery-faults",
    "install-upgrade-rollback", "code-signing", "sbom-licenses", "privacy-defaults",
    "office-exchange", "reproducible-builds", "xinchuang-hardware",
}


def audit(root: Path, checklist: dict) -> dict:
    tasks_text = (root / "docs/joyword/TASKS.md").read_text(encoding="utf-8")
    tasks = []
    blockers = []
    invalid = []
    for task_id, title, body in TASK.findall(tasks_text):
        if int(task_id[1:]) >= 800:
            continue
        states = STATE.findall(body)
        if len(states) != 1 or (states[0][0] == "x") != (states[0][1] == "done"):
            invalid.append(f"{task_id}: invalid task state")
            continue
        state = states[0][1]
        nonblocking = task_id == "T005"
        tasks.append({"id": task_id, "title": title, "status": state, "nonblocking": nonblocking})
        if state != "done" and not nonblocking:
            blockers.append(f"{task_id}: {state}")
    expected_tasks = {f"T{number:03}" for number in [
        *range(0, 6), *range(100, 112), *range(200, 212), *range(300, 305),
        *range(400, 404), *range(500, 505), *range(600, 606), *range(700, 705)]}
    if {task["id"] for task in tasks} != expected_tasks:
        invalid.append("M0-M7 task set is incomplete or contains unexpected tasks")
    if checklist.get("schema") != 1:
        invalid.append("unsupported checklist schema")
    observations = checklist.get("observations", [])
    if not isinstance(observations, list) or any(not isinstance(item, dict) for item in observations):
        raise ValueError("observations must be an array of records")
    ids = [item.get("id") for item in observations]
    if set(ids) != REQUIRED or len(ids) != len(REQUIRED):
        invalid.append("release observation set is incomplete or duplicated")
    for item in observations:
        name = item.get("id")
        state = item.get("status")
        nonblocking = name == "xinchuang-hardware"
        if state not in ("pending", "failed", "passed") or not item.get("scope"):
            invalid.append(f"{name}: invalid status or missing scope")
        if state == "passed":
            evidence = item.get("evidence", [])
            if not evidence:
                invalid.append(f"{name}: passed without evidence")
            for record in evidence:
                path = root / record.get("path", "")
                try:
                    path.resolve().relative_to(root.resolve())
                except ValueError:
                    invalid.append(f"{name}: evidence escapes repository")
                    continue
                if not path.is_file() or digest(path) != record.get("sha256"):
                    invalid.append(f"{name}: missing or changed evidence {record.get('path')}")
        elif not item.get("reason"):
            invalid.append(f"{name}: pending/failed observation requires a reason")
        if state != "passed" and not nonblocking:
            blockers.append(f"{name}: {state}")
    return {"schema": 1, "release_ready": not blockers and not invalid,
            "tasks": tasks, "observations": observations,
            "blockers": blockers, "invalid": invalid,
            "policy": "Xinchuang hardware pending is nonblocking, never passed by substitution"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checklist", type=Path, default=ROOT / "docs/joyword/release/checklist.json")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--audit", action="store_true")
    args = parser.parse_args()
    try:
        report = audit(ROOT, json.loads(args.checklist.read_text(encoding="utf-8")))
    except (ValueError, OSError, TypeError, KeyError) as error:
        print(f"invalid release evidence: {error}")
        return 2
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"release_ready={report['release_ready']}; {len(report['blockers'])} pending gates; {len(report['invalid'])} invalid records")
    for problem in report["invalid"]:
        print(problem)
    return 2 if report["invalid"] else 0 if args.audit or report["release_ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
