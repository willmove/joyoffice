#!/usr/bin/env python3
"""Check JoyWord acceptance checklists against TASKS.md and real test names.

Stdlib only. Reuses ``rust_function_names`` from ``check_docs`` and does not
modify that module. A backticked token in an evidence cell is a test name when
it is a single Rust identifier containing an underscore. CI job names and file
names are not written that way.
"""
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_docs  # noqa: E402

LEGEND = (
    "automated",
    "partial",
    "not-started",
    "needs-gui",
    "needs-word-wps",
    "needs-hardware",
    "not-implemented",
)
TEST_NAME = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
TASK_SPLIT = re.compile(r"^### (T\d{3})：[^\n]+\n", re.M)
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
FENCE = re.compile(r"^```", re.M)


def _cells(line):
    text = line.strip()
    if not text.startswith("|"):
        return None
    return [cell.strip() for cell in text.strip("|").split("|")]


def _tables(text):
    lines = text.splitlines()
    index = 0
    while index < len(lines):
        header = _cells(lines[index])
        if header and index + 1 < len(lines) and re.fullmatch(r"\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?", lines[index + 1].strip()):
            index += 2
            while index < len(lines):
                row = _cells(lines[index])
                if row is None:
                    break
                yield header, row
                index += 1
            continue
        index += 1


def _task_quotes(tasks_text):
    parts = TASK_SPLIT.split(tasks_text)
    quotes = {}
    for index in range(1, len(parts), 2):
        task_id, body = parts[index], parts[index + 1]
        found = {}
        for field in ("自动验证", "人工验收"):
            matches = re.findall(rf"^- {field}：(.+)$", body, re.M)
            if len(matches) == 1:
                found[field] = matches[0]
        if len(found) == 2:
            quotes[task_id] = found
    return quotes


def _unfenced(text):
    """Drop fenced code so a sample checklist in a guide is not parsed."""
    chunks = FENCE.split(text)
    return "".join(chunk for index, chunk in enumerate(chunks) if index % 2 == 0)


def problems(root, test_names=None):
    """Return human-readable problems for acceptance docs under ``root``."""
    root = Path(root)
    docs = root / "docs" / "joyword"
    acceptance = docs / "acceptance"
    found = []
    files = sorted(acceptance.glob("T*.md")) if acceptance.is_dir() else []
    if not files:
        return ["acceptance: no T*.md checklists"]
    readme = acceptance / "README.md"
    if not readme.is_file():
        found.append("acceptance: missing README.md legend")
        legend_text = ""
    else:
        legend_text = readme.read_text(encoding="utf-8")
        for status in LEGEND:
            if not re.search(rf"(?<![A-Za-z0-9-]){status}(?![A-Za-z0-9-])", legend_text):
                found.append(f"acceptance/README.md: legend missing status {status}")
    tasks_path = docs / "TASKS.md"
    if not tasks_path.is_file():
        found.append("acceptance: TASKS.md is missing")
        quotes = {}
    else:
        quotes = _task_quotes(tasks_path.read_text(encoding="utf-8"))
    if test_names is None:
        test_names = check_docs.rust_function_names([root / "crates", root / "apps"])

    for path in files:
        task_id = path.stem
        label = f"docs/joyword/acceptance/{path.name}"
        text = path.read_text(encoding="utf-8")
        if task_id not in quotes:
            found.append(f"{label}: {task_id} has no 自动验证/人工验收 in TASKS.md")
        else:
            for field, quoted in quotes[task_id].items():
                if quoted not in text:
                    found.append(f"{label}: does not cite {field} text")
        if "## GUI 抽查脚本" not in text:
            found.append(f"{label}: missing GUI spot-check heading")
        saw_table = False
        for header, row in _tables(text):
            if "状态" not in header or "证据" not in header:
                continue
            saw_table = True
            if len(row) != len(header):
                found.append(f"{label}: table row has {len(row)} cells, header has {len(header)}")
                continue
            status = row[header.index("状态")]
            evidence = row[header.index("证据")]
            if status not in LEGEND:
                found.append(f"{label}: status {status!r} is not in the legend")
            for token in re.findall(r"`([^`]+)`", evidence):
                if TEST_NAME.fullmatch(token) and "_" in token and token not in test_names:
                    found.append(f"{label}: evidence cites missing test {token}")
        if not saw_table:
            found.append(f"{label}: no criteria table with 状态 and 证据 columns")
        for target in LINK.findall(_unfenced(text)):
            parsed = urlsplit(target.strip("<>"))
            if parsed.scheme or not parsed.path:
                continue
            raw = unquote(parsed.path)
            if Path(raw).is_absolute():
                found.append(f"{label}: absolute link {raw}")
                continue
            local = (path.parent / raw).resolve()
            try:
                local.relative_to(root.resolve())
            except ValueError:
                found.append(f"{label}: link escapes the repository: {raw}")
                continue
            if not local.is_file():
                found.append(f"{label}: missing linked file {raw}")
    return found


def main():
    root = Path(__file__).resolve().parents[1]
    found = problems(root)
    if found:
        for item in found:
            print(f"ERROR: {item}", file=sys.stderr)
        return 1
    count = len(list((root / "docs/joyword/acceptance").glob("T*.md")))
    print(f"OK: {count} acceptance checklists; legend statuses; cited tests exist.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
