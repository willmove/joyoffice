#!/usr/bin/env python3
"""Standalone crate registration and Action pins until JoyOffice integration.

The unified workspace must replace this registry with JoyOffice's LAYERS,
DEV_LAYERS, filesystem and panic gates; this is not a module-layer audit.
"""

from pathlib import Path
import re
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
# No internal production/dev/build edges are currently needed.
CRATES = {"apps/joydeck": "joydeck", "xtask": "xtask"}


def check(root: Path) -> list[str]:
    errors = []
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    if set(workspace["members"]) != set(CRATES):
        errors.append("workspace members differ from the reviewed crate registry")
    manifests = {*(root / "apps").glob("*/Cargo.toml"),
                 *(root / "crates").glob("*/Cargo.toml"), root / "xtask/Cargo.toml"}
    for manifest in sorted(manifests):
        path = manifest.parent.relative_to(root).as_posix()
        data = tomllib.loads(manifest.read_text())
        name = data.get("package", {}).get("name")
        if CRATES.get(path) != name:
            errors.append(f"unregistered crate {name} at {path}")
        if data.get("lints", {}).get("workspace") is not True:
            errors.append(f"{path} must inherit workspace lints")
        for scope in [data, *data.get("target", {}).values()]:
            for tier in ("dependencies", "dev-dependencies", "build-dependencies"):
                for key, spec in scope.get(tier, {}).items():
                    package = spec.get("package", key) if isinstance(spec, dict) else key
                    if package in CRATES.values() or package.startswith(("joyword-", "joyoffice-", "joydeck-")) or package == "joyword":
                        errors.append(f"{name} has an unreviewed internal dependency: {package}")
                    if isinstance(spec, dict) and "path" in spec:
                        errors.append(f"{name} has an unreviewed path dependency: {key}")
    if workspace.get("lints", {}).get("rust", {}).get("unsafe_code") != "forbid":
        errors.append("workspace must forbid unsafe_code")
    for workflow in sorted((root / ".github/workflows").glob("*.y*ml")):
        for match in re.finditer(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)", workflow.read_text(), re.M):
            ref = match[1].strip("\"'")
            if not ref.startswith("./") and not re.fullmatch(r"[^@]+@[0-9a-f]{40}", ref):
                errors.append(f"{workflow.name}: Action must use a full commit SHA: {ref}")
    return errors


if __name__ == "__main__":
    problems = check(ROOT)
    for problem in problems:
        print(problem, file=sys.stderr)
    if not problems:
        print("Repository gates OK: 2 registered crates, workspace lints, Action SHAs")
    sys.exit(bool(problems))
