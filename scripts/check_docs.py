#!/usr/bin/env python3
"""Validate specification traceability and fixture metadata, not product compatibility."""
import datetime
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
# T205: evidenced ribbon, ruler and status controls. Lowering this is a coverage regression.
FEATURE_FLOOR = 50
# T209: undo-group rules (SPEC §6) that must stay registered with a test.
REQUIRED_BEHAVIORS = (
    "EDIT-UNDO-TYPING",
    "EDIT-UNDO-DELETING",
    "EDIT-UNDO-SOURCE-BOUNDARY",
    "EDIT-UNDO-CARET-BOUNDARY",
    "EDIT-UNDO-REPLACE-ELSEWHERE",
    "EDIT-UNDO-FORMAT-BOUNDARY",
    "EDIT-UNDO-AFTER-HISTORY",
    "EDIT-UNDO-ORIGIN",
    "EDIT-UNDO-TYPING-CAP",
    "EDIT-UNDO-FAILED-INPUT",
)
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


ISO_DATE = r"\d{4}-\d{2}-\d{2}"
HASH64 = r"[0-9a-f]{64}"


def _valid_date(value):
    try:
        datetime.date.fromisoformat(value)
        return True
    except ValueError:
        return False


def rust_function_names(roots):
    """Every Rust function name in the given trees (test names included)."""
    names = set()
    for root in roots:
        if not root.is_dir():
            continue
        for source in root.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            names.update(re.findall(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*[<(]", text))
    return names


def validate_behavior_registry(path, test_names, required=()):
    """T004 editing-behavior registry: schema, hypothesis/rule discipline,
    and every referenced test name must exist in the workspace. Each ID in
    `required` must be registered (T209 undo-group rules)."""
    problems = []
    if not path.is_file():
        return [f"behavior registry missing: {path}"], 0
    try:
        registry = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        return [f"behavior registry is not valid JSON: {error}"], 0
    if not isinstance(registry, dict) or registry.get("schema") != 1:
        return ["behavior registry: unsupported schema"], 0
    entries = registry.get("registry")
    if not isinstance(entries, list) or not entries:
        return ["behavior registry: empty or missing entries"], 0
    seen = set()
    count = 0
    for index, entry in enumerate(entries, 1):
        label = f"behavior registry entry {index}"
        if not isinstance(entry, dict):
            problems.append(f"{label}: not an object")
            continue
        count += 1
        entry_id = entry.get("id")
        if not isinstance(entry_id, str) or not re.fullmatch(r"[A-Z0-9-]+", entry_id):
            problems.append(f"{label}: invalid id")
        elif entry_id in seen:
            problems.append(f"{label}: duplicate id {entry_id}")
        else:
            seen.add(entry_id)
        for field in ("operation", "expectation", "test"):
            if not isinstance(entry.get(field), str) or not entry[field].strip():
                problems.append(f"{label}: missing {field}")
        source = entry.get("source")
        if source not in ("hypothesis", "word"):
            problems.append(f"{label}: source must be hypothesis or word")
            continue
        observation = entry.get("word_observation")
        if source == "hypothesis":
            if observation is not None:
                problems.append(f"{label}: a hypothesis must not claim a Word observation")
        else:
            if not isinstance(observation, dict):
                problems.append(f"{label}: a Word rule needs word_observation")
            else:
                for field in ("version", "sample_id"):
                    if not isinstance(observation.get(field), str) or not observation[field].strip():
                        problems.append(f"{label}: word_observation missing {field}")
                if not isinstance(observation.get("date"), str) \
                        or not re.fullmatch(ISO_DATE, observation["date"]) \
                        or not _valid_date(observation["date"]):
                    problems.append(f"{label}: word_observation date must be YYYY-MM-DD")
                digest = observation.get("screenshot_sha256")
                if not isinstance(digest, str) or not re.fullmatch(HASH64, digest):
                    problems.append(f"{label}: word_observation needs a 64-hex screenshot SHA-256")
        test = entry.get("test")
        if isinstance(test, str) and test and test not in test_names:
            problems.append(f"{label}: test {test} not found in the workspace")
    for entry_id in required:
        if entry_id not in seen:
            problems.append(f"behavior registry: required entry {entry_id} missing")
    return problems, count


def validate_feature_catalog(requirements):
    """T205: shown controls bind a requirement and an evidence level, and icons are drawn in code."""
    path = DOCS / "feature_catalog.json"
    catalog = json.loads(path.read_text(encoding="utf-8"))
    check(catalog.get("schema_version") == 1, "feature catalog: schema_version must be 1")
    check(catalog.get("task") == "T205", "feature catalog: task must be T205")
    check(catalog.get("floor") == FEATURE_FLOOR,
          f"feature catalog: floor must stay at {FEATURE_FLOOR}")
    controls = catalog.get("controls")
    icons = catalog.get("icons")
    check(isinstance(controls, list) and isinstance(icons, list),
          "feature catalog: controls and icons must be lists")
    if not isinstance(controls, list) or not isinstance(icons, list):
        return
    command_ids = set(re.findall(
        r'CommandId\("([a-z0-9_.]+)"\)',
        (ROOT / "crates/joyword-shell/src/command.rs").read_text(encoding="utf-8")))
    chrome = (ROOT / "crates/joyword-shell/src/chrome.rs").read_text(encoding="utf-8")
    icons_src = chrome.split("const ICONS:", 1)
    check(len(icons_src) == 2, "feature catalog: chrome.rs is missing the ICONS table")
    # rustfmt wraps each ("id", "art") pair, so the id and the art may sit on
    # different lines. Only the ICONS table is scanned.
    drawn = set(re.findall(r'"([a-z0-9-]+)",\s*"', icons_src[-1], re.S))
    icon_ids = set()
    for icon in icons:
        icon_id = icon.get("id")
        check(isinstance(icon_id, str) and icon_id and icon_id not in icon_ids,
              f"feature catalog: duplicate or empty icon {icon_id}")
        icon_ids.add(icon_id)
        check(icon.get("origin") == "drawn-in-code",
              f"feature catalog: {icon_id} must be drawn-in-code")
        check(icon.get("license") == "MIT", f"feature catalog: {icon_id} license must be MIT")
        source = icon.get("source")
        check(source == "crates/joyword-shell/src/chrome.rs" and (ROOT / source).is_file(),
              f"feature catalog: {icon_id} source is not the icon drawing")
        check(icon_id in drawn, f"feature catalog: {icon_id} has no drawing in chrome.rs")
    evidence_levels = {"exists", "unit", "corpus", "reference"}
    seen = set()
    evidenced = 0
    for control in controls:
        control_id = control.get("id")
        check(isinstance(control_id, str) and control_id and control_id not in seen,
              f"feature catalog: duplicate or empty control {control_id}")
        seen.add(control_id)
        kind = control.get("kind")
        check(kind in {"command", "status", "ruler"},
              f"feature catalog: {control_id} has an unknown kind")
        requirement = control.get("requirement")
        check(requirement in requirements,
              f"feature catalog: {control_id} has an unknown requirement")
        evidence = control.get("evidence")
        check(evidence in evidence_levels,
              f"feature catalog: {control_id} needs an evidence level")
        if evidence in evidence_levels:
            evidenced += 1
        if kind == "command":
            check(control.get("command") in command_ids,
                  f"feature catalog: {control_id} command is not registered")
            icon = control.get("icon")
            check(icon in icon_ids, f"feature catalog: {control_id} icon is not attributed")
            check(control.get("tab") in {"start", "insert", "layout", "review", "view"},
                  f"feature catalog: {control_id} tab is not a ribbon tab")
        else:
            check("command" not in control and "icon" not in control,
                  f"feature catalog: {control_id} is not a command and must not carry one")
    check(evidenced == FEATURE_FLOOR,
          f"feature catalog: evidenced controls {evidenced} != floor {FEATURE_FLOOR}")
    for suffix in (".png", ".svg", ".ico", ".jpg", ".jpeg", ".webp", ".gif", ".bmp",
                   ".ttf", ".otf", ".woff", ".woff2"):
        for root in (ROOT / "apps/joyword", ROOT / "crates/joyword-shell"):
            for asset in root.rglob(f"*{suffix}"):
                if asset.is_file():
                    check(False, f"feature catalog: unattributed binary asset {asset.relative_to(ROOT)}")


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
    task_statuses = {}
    covered = set()
    req_tasks = {}
    status_names = ["done", "in_progress", "planned", "blocked"]
    fields = ["状态", "需求", "前置", "输入", "产物", "错误路径", "自动验证", "人工验收", "完成证据"]
    for index in range(1, len(blocks), 2):
        task_id, body = blocks[index:index + 2]
        check(task_id not in tasks, f"TASKS: duplicate ID {task_id}")
        values = {}
        for field in fields:
            if field == "状态":
                matches = re.findall(r"^- \[([ x])\] 状态：(.+)$", body, re.M)
                check(len(matches) == 1, f"{task_id}: missing or duplicate field {field}")
                checkbox, values[field] = matches[0] if matches else ("", "")
                check((values[field] == "`done`") == (checkbox == "x"),
                      f"{task_id}: checkbox must be [x] only for done")
            else:
                matches = re.findall(rf"^- {field}：(.+)$", body, re.M)
                check(len(matches) == 1, f"{task_id}: missing or duplicate field {field}")
                values[field] = matches[0] if matches else ""
        check(values["状态"] in [f"`{s}`" for s in status_names],
              f"{task_id}: invalid status")
        task_statuses[task_id] = values["状态"].strip("`")
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

    summary_rows = re.findall(
        r"^\| (M\d) \| (\d+) \| (\d+) \| (\d+) \| (\d+) \| (\d+) \|$", task_text, re.M)
    summary = {stage: tuple(map(int, counts)) for stage, *counts in summary_rows}
    check(len(summary) == len(summary_rows), "TASKS: duplicate progress summary row")
    for stage_number in range(10):
        stage = f"M{stage_number}"
        statuses = [status for task, status in task_statuses.items() if int(task[1]) == stage_number]
        expected = tuple(statuses.count(status) for status in status_names) + (len(statuses),)
        check(summary.get(stage) == expected,
              f"TASKS: progress summary mismatch for {stage}; expected {expected}")
    total_rows = re.findall(r"^\| 总计 \| (\d+) \| (\d+) \| (\d+) \| (\d+) \| (\d+) \|$",
                            task_text, re.M)
    statuses = list(task_statuses.values())
    expected_total = tuple(statuses.count(status) for status in status_names) + (len(statuses),)
    check(len(total_rows) == 1 and tuple(map(int, total_rows[0])) == expected_total,
          f"TASKS: progress total mismatch; expected {expected_total}")

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
    ui_sources = [*sorted((ROOT / "apps/joyword/src").glob("*.rs")), *sorted((ROOT / "crates/joyword-shell/src").glob("*.rs"))]
    used_keys = set()
    for source in ui_sources:
        text = source.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
        used_keys |= set(re.findall(r'(?:label|text|format|format_count|Disabled)\(\s*"([a-z][a-z0-9_.]*)"', text))
        used_keys |= set(re.findall(r'(?:label|group_key): "([a-z][a-z0-9_.]*)"', text))
        used_keys |= set(re.findall(r'=> "(ribbon\.tab\.[a-z0-9_.]+)"', text))
    missing = sorted(used_keys - (locale_keys[0] & locale_keys[1]))
    check(not missing, f"locales: UI references missing keys {missing}")
    validate_feature_catalog(requirements)

    workflows = sorted((ROOT / ".github/workflows").glob("*.yml"))
    actions = 0
    for workflow in workflows:
        text = workflow.read_text(encoding="utf-8")
        for ref in re.findall(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)", text, re.M):
            actions += 1
            check(ref.startswith("./") or bool(re.fullmatch(r"[\w.-]+/[\w./-]+@[0-9a-f]{40}", ref)),
                  f"{workflow.relative_to(ROOT)}: action not pinned to a full commit SHA: {ref}")

    test_names = rust_function_names([ROOT / "crates", ROOT / "apps"])
    registry_problems, behavior_count = validate_behavior_registry(
        DOCS / "behavior_registry.json", test_names, REQUIRED_BEHAVIORS)
    errors.extend(registry_problems)

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"OK: {len(requirements)} requirements; {len(tasks)} tasks; acyclic dependencies; "
          f"stage-consistent; M0–M7 required by {RELEASE_GATE}; "
          f"{len(markdown)} Markdown files; {len(case_ids)} registered cases; locale parity; "
          f"{actions} workflow actions pinned by SHA; "
          f"{behavior_count} behavior registry entries; "
          f"feature catalog floor {FEATURE_FLOOR}.")
    print("Metadata validation only; Office compatibility and GUI behavior are not tested here.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(validate())
    except (KeyError, TypeError, ValueError, OSError) as error:
        sys.exit(f"ERROR: malformed or missing metadata: {error}")
