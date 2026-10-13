#!/usr/bin/env python3
"""Validate specification traceability and fixture metadata, not product compatibility."""
import hashlib
import json
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs/joyword"
REQUIREMENT = r"(?:EDIT|STYLE|LAYOUT|REVIEW|FORMAT|CLIP|AI|PLATFORM|QUALITY|COLLAB|SUITE)-\d{3}"
RELEASE_GATE = "T704"  # Every M0–M7 task must be a prerequisite of the 1.0 gate.
errors = []


def check(condition, message):
    if not condition:
        errors.append(message)


def local_file(base, value, context):
    """Keep registry paths repository-relative, including through symlinks."""
    path = (base / value).resolve()
    check(not Path(value).is_absolute() and path.is_relative_to(ROOT),
          f"{context}: path must remain inside repository: {value}")
    check(path.is_file(), f"{context}: missing file: {value}")
    return path


def validate():
    spec = (DOCS / "SPEC.md").read_text(encoding="utf-8")
    req_list = re.findall(rf"^\| ({REQUIREMENT}) \|", spec, re.M)
    requirements = set(req_list)
    check(len(req_list) == len(requirements) and bool(requirements),
          "SPEC: duplicate or missing requirement IDs")
    stages = dict(re.findall(rf"^\| ({REQUIREMENT}) \|.+\| M(\d) \|$", spec, re.M))
    check(set(stages) == requirements, "SPEC: every requirement needs one M0–M9 stage")

    task_text = (DOCS / "TASKS.md").read_text(encoding="utf-8")
    blocks = re.split(r"^### (T\d{3})：[^\n]+\n", task_text, flags=re.M)
    tasks = {}
    covered = set()
    req_tasks = {}
    fields = ["状态", "需求", "前置", "输入", "产物", "错误路径", "自动验证", "人工验收", "完成证据"]
    for index in range(1, len(blocks), 2):
        task_id, body = blocks[index:index + 2]
        check(task_id not in tasks, f"TASKS: duplicate ID {task_id}")
        values = {}
        for field in fields:
            matches = re.findall(rf"^- {field}：(.+)$", body, re.M)
            check(len(matches) == 1, f"{task_id}: missing or duplicate field {field}")
            values[field] = matches[0] if matches else ""
        check(values["状态"] in [f"`{s}`" for s in ["planned", "in_progress", "blocked", "done"]],
              f"{task_id}: invalid status")
        if values["状态"] == "`done`":
            check(bool(re.search(r"\[[^]]+\]\([^)]+\)", values["完成证据"])),
                  f"{task_id}: done requires a linked evidence record")
        reqs = set(re.findall(REQUIREMENT, values["需求"]))
        check(bool(reqs) and reqs <= requirements, f"{task_id}: unknown or missing requirement")
        covered.update(reqs)
        for req in reqs:
            req_tasks.setdefault(req, []).append(task_id)
        deps = re.findall(r"T\d{3}", values["前置"])
        check(bool(deps) or values["前置"] == "无", f"{task_id}: invalid dependencies")
        check(len(deps) == len(set(deps)), f"{task_id}: duplicate dependencies")
        tasks[task_id] = deps
    check(bool(tasks), "TASKS: no tasks found")
    check(covered == requirements, f"TASKS: unmapped requirements {sorted(requirements - covered)}")
    for req, stage in stages.items():
        milestones = [int(task_id[1]) for task_id in req_tasks.get(req, [])]
        check(bool(milestones) and min(milestones) <= int(stage),
              f"{req}: no task at or before its stage M{stage}")

    visited, visiting = set(), set()

    def visit(task_id):
        if task_id in visiting:
            errors.append(f"TASKS: dependency cycle at {task_id}")
            return
        if task_id in visited:
            return
        visiting.add(task_id)
        for dep in tasks[task_id]:
            if dep not in tasks:
                errors.append(f"{task_id}: unknown dependency {dep}")
            else:
                visit(dep)
        visiting.remove(task_id)
        visited.add(task_id)

    for task_id in tasks:
        visit(task_id)

    check(RELEASE_GATE in tasks, f"TASKS: missing release gate {RELEASE_GATE}")
    required, pending = {RELEASE_GATE}, [RELEASE_GATE]
    while pending:
        for dep in tasks.get(pending.pop(), []):
            if dep in tasks and dep not in required:
                required.add(dep)
                pending.append(dep)
    outside = sorted(task_id for task_id in tasks if int(task_id[1]) <= 7 and task_id not in required)
    check(not outside, f"TASKS: M0–M7 tasks not required by {RELEASE_GATE}: {outside}")

    markdown = [ROOT / "README.md", *DOCS.rglob("*.md"), ROOT / "tests/fixtures/README.md"]
    for path in markdown:
        for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", path.read_text(encoding="utf-8")):
            parsed = urlsplit(target.strip("<>"))
            if not parsed.scheme and parsed.path:
                local_file(path.parent, unquote(parsed.path), str(path.relative_to(ROOT)))

    fixtures = ROOT / "tests/fixtures"
    registry = json.loads((fixtures / "manifest.json").read_text(encoding="utf-8"))
    check(registry["schema_version"] == 1, "fixtures: unsupported schema version")
    case_ids = set()
    for case in registry["cases"]:
        case_id = case["id"]
        check(case_id not in case_ids, f"fixtures: duplicate ID {case_id}")
        case_ids.add(case_id)
        check(case["kind"] in ["text_seed", "format_case"], f"{case_id}: invalid kind")
        check(bool(case["origin"]) and bool(case["license"]), f"{case_id}: missing provenance")
        check(bool(case["requirements"]) and set(case["requirements"]) <= requirements,
              f"{case_id}: unknown or missing requirement")
        check(bool(case["expected"]), f"{case_id}: missing expected behavior")
        check(case["status"] in ["planned", "passed", "failed", "blocked"], f"{case_id}: invalid status")
        if case["kind"] == "text_seed":
            check(case["status"] == "planned", f"{case_id}: a text seed cannot claim compatibility")
        if case["kind"] == "format_case":
            local_file(fixtures, case["document"], case_id)
            profile = case["reference_profile"]
            check(isinstance(profile, dict) and profile.get("status") in ["unverified", "established"],
                  f"{case_id}: reference profile needs status unverified or established")
            # A missing reference result keeps a case unverified; it cannot pass.
            check(case["status"] != "passed" or profile.get("status") == "established",
                  f"{case_id}: passed without an established reference result")
        if case["status"] in ["passed", "failed"]:
            check(bool(case["evidence"]), f"{case_id}: result needs evidence")
        for evidence in case["evidence"]:
            local_file(fixtures, evidence, case_id)
        path = local_file(fixtures, case["path"], case_id)
        if path.is_file() and path.is_relative_to(ROOT):
            check(hashlib.sha256(path.read_bytes()).hexdigest() == case["sha256"],
                  f"{case_id}: input SHA-256 mismatch")

    # UI strings live in joyword-shell; plural variants `key[one]` count as
    # one base key. Full parity, placeholder and CLDR checks run in Rust tests.
    locale_keys = []
    for locale in ["zh-CN", "en"]:
        resource = ROOT / f"crates/joyword-shell/locales/{locale}.lang"
        keys = set()
        seen = set()
        for line in resource.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            key, separator, value = line.partition("=")
            key = key.strip()
            check(bool(separator) and bool(value.strip()), f"{locale}: invalid resource line")
            check(key not in seen, f"{locale}: duplicate key {key}")
            seen.add(key)
            keys.add(re.sub(r"\[(one|other)\]$", "", key))
        locale_keys.append(keys)
    check(locale_keys[0] == locale_keys[1], "locales: Chinese and English key sets differ")
    ui_sources = [ROOT / "apps/joyword/src/main.rs", *sorted((ROOT / "crates/joyword-shell/src").glob("*.rs"))]
    used_keys = set()
    for source in ui_sources:
        text = source.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
        used_keys |= set(re.findall(r'(?:label|text|format|format_count|Disabled)\(\s*"([a-z][a-z0-9_.]*)"', text))
        used_keys |= set(re.findall(r'label: "([a-z][a-z0-9_.]*)"', text))
    missing = sorted(used_keys - (locale_keys[0] & locale_keys[1]))
    check(not missing, f"locales: UI references missing keys {missing}")

    workflows = sorted((ROOT / ".github/workflows").glob("*.yml"))
    actions = 0
    for workflow in workflows:
        text = workflow.read_text(encoding="utf-8")
        for ref in re.findall(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)", text, re.M):
            actions += 1
            check(ref.startswith("./") or bool(re.fullmatch(r"[\w.-]+/[\w./-]+@[0-9a-f]{40}", ref)),
                  f"{workflow.relative_to(ROOT)}: action not pinned to a full commit SHA: {ref}")

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"OK: {len(requirements)} requirements; {len(tasks)} tasks; acyclic dependencies; "
          f"stage-consistent; M0–M7 required by {RELEASE_GATE}; "
          f"{len(markdown)} Markdown files; {len(case_ids)} registered cases; locale parity; "
          f"{actions} workflow actions pinned by SHA.")
    print("Metadata validation only; Office compatibility and GUI behavior are not tested here.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(validate())
    except (KeyError, TypeError, ValueError, OSError) as error:
        sys.exit(f"ERROR: malformed or missing metadata: {error}")
