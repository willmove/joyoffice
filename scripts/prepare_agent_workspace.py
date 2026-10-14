#!/usr/bin/env python3
"""Prepare one assigned desktop-agent workspace; never reset existing work."""

import argparse
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]


def git(*args, cwd=ROOT):
    return subprocess.run(
        ["git", *map(str, args)], cwd=str(cwd), text=True,
        encoding="utf-8", errors="replace", capture_output=True, check=True,
    ).stdout.strip()


def branch_exists(repo, branch):
    result = subprocess.run(
        ["git", "show-ref", "--verify", "--quiet", "refs/heads/" + branch],
        cwd=str(repo), capture_output=True,
    )
    if result.returncode not in (0, 1):
        raise RuntimeError("Cannot inspect local branches")
    return result.returncode == 0


def display(command):
    if os.name == "nt":
        return subprocess.list2cmdline(command)
    return shlex.join(command)


def prepare(args):
    plan = json.loads((ROOT / "docs/coordination/plan.json").read_text(encoding="utf-8"))
    tasks = {task["id"]: task for task in plan["tasks"]}
    if args.task not in tasks:
        raise ValueError("Unknown task; choose " + ", ".join(sorted(tasks)))
    task = tasks[args.task]
    source = Path(git("rev-parse", "--show-toplevel")).resolve()
    destination = Path(args.destination).expanduser().resolve() if args.destination else source.parent / task["directory"]
    if destination.exists() or destination.is_symlink():
        raise ValueError("Destination already exists; preserve it and inspect before resuming: " + str(destination))
    if destination == source or source in destination.parents or destination in source.parents:
        raise ValueError("Destination must be outside the source checkout and its ancestors")
    if not destination.parent.is_dir():
        raise ValueError("Destination parent does not exist: " + str(destination.parent))
    if branch_exists(source, task["branch"]):
        raise ValueError("Task branch already exists locally; inspect the existing task instead of starting a duplicate")
    ref = plan["coordination_ref"]
    if not ref.startswith("refs/heads/"):
        raise ValueError("Coordination ref must name a branch")
    git("remote", "get-url", "origin")
    print("Task:", args.task)
    print("Source:", source)
    print("Destination:", destination)
    print("Branch:", task["branch"])
    print("Mode:", args.mode)
    print("Code baseline:", plan["code_base"])
    if args.dry_run:
        print("Dry run: no fetch, branch, clone or directory creation performed.")
        print(display(["git", "fetch", "--no-tags", "origin", ref]))
        print("Then verify ancestry and create an isolated", args.mode, "at the fetched commit.")
        return 0

    remote_branch = subprocess.run(
        ["git", "ls-remote", "--exit-code", "--heads", "origin", task["branch"]],
        cwd=str(source), capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if remote_branch.returncode == 0:
        raise ValueError("Task branch already exists on origin; ask the coordinator whether to resume, not duplicate")
    if remote_branch.returncode != 2:
        raise RuntimeError("Cannot verify task ownership on origin: " + remote_branch.stderr.strip())

    git("fetch", "--no-tags", "origin", ref, cwd=source)
    start = git("rev-parse", "FETCH_HEAD", cwd=source)
    git("merge-base", "--is-ancestor", plan["code_base"], start, cwd=source)
    card = Path(task["card"])
    if card.is_absolute() or ".." in card.parts:
        raise ValueError("Task card must remain repository-relative")
    git("cat-file", "-e", start + ":" + task["card"], cwd=source)
    if args.mode == "worktree":
        git("worktree", "add", "-b", task["branch"], destination, start, cwd=source)
    else:
        origin = git("remote", "get-url", "origin", cwd=source)
        git("clone", "--no-checkout", "--single-branch", "--branch", ref.removeprefix("refs/heads/"), origin, destination, cwd=source)
        git("switch", "-c", task["branch"], start, cwd=destination)
    if git("rev-parse", "HEAD", cwd=destination) != start:
        raise RuntimeError("Unexpected result commit; preserve the directory for inspection")
    print("Prepared HEAD:", start)
    print("Open THIS directory in the desktop Agent:", destination)
    print("Task card:", destination / task["card"])
    print("Report:", destination / "docs/coordination/reports" / (args.task + ".md"))
    print("No Agent session was started. Use the launch prompt in docs/coordination/README.md.")
    print("If this task was already started elsewhere, stop here and contact the coordinator.")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", required=True, help="Assigned task ID, e.g. W01")
    parser.add_argument("--mode", choices=("worktree", "clone"), default="worktree")
    parser.add_argument("--destination", help="New task directory, default: sibling joyoffice-wXX")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    try:
        return prepare(args)
    except subprocess.CalledProcessError as error:
        print("ERROR: git operation failed:", error.stderr.strip(), file=sys.stderr)
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        print("ERROR:", error, file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
