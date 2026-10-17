#!/usr/bin/env python3
"""T111 gates: no-panic baseline, crate layers, and filesystem allowlist.

A counterexample is any extra argument that is a source file the checks must
reject. The committed baseline lists existing unwrap/expect/panic sites.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
APPS = ROOT / "apps"

# Direct dependencies that match ARCHITECTURE §1. Paths are crate directories.
LAYERS = {
    "joyword-foundation": set(),
    "joyword-model": {"joyword-foundation"},
    "joyword-engine": {"joyword-model", "joyword-foundation"},
    "joyoffice-opc": set(),
    "joyword-docx": {"joyoffice-opc", "joyword-model", "joyword-foundation"},
    "joyword-layout": {"joyword-foundation", "joyword-model"},
    "joyword-render": {"joyword-foundation", "joyword-layout"},
    "joyword-pdf": {"joyword-foundation", "joyword-layout"},
    "joyword-corpus": {"joyoffice-opc"},
    "joyword-shell": {
        "joyoffice-opc",
        "joyword-docx",
        "joyword-engine",
        "joyword-model",
    },
    "joyword": {
        "joyword-engine",
        "joyword-foundation",
        "joyword-layout",
        "joyword-model",
        "joyword-render",
        "joyword-shell",
    },
}

FS_ALLOW = {
    "crates/joyoffice-opc/src/zip.rs",
    "crates/joyoffice-opc/src/package.rs",
    "crates/joyword-shell/src/storage.rs",
    "crates/joyword-shell/src/storage/source.rs",
    "crates/joyword-shell/src/storage/original.rs",
    "crates/joyword-shell/src/storage/locks.rs",
    "crates/joyword-shell/src/storage/recovery.rs",
    "crates/joyword-shell/src/storage/metadata.rs",
    "crates/joyword-shell/src/prefs.rs",
    "crates/joyword-shell/src/docx.rs",
    "crates/joyword-shell/src/host.rs",
    "apps/joyword/src/main.rs",
    "apps/joyword/src/document_canvas.rs",
    "apps/joyword/src/input_ui.rs",
}

PANIC_RE = re.compile(
    r"(?:\.unwrap\s*\(|\.expect\s*\(|\bpanic!\s*\(|\bunreachable!\s*\()"
)
FS_RE = re.compile(r"\b(?:std::fs::|fs::(?:read|write|create_dir|remove_file|rename|copy|metadata|File::))")
DEP_RE = re.compile(r"^([A-Za-z0-9_-]+)\s*=")


def crate_name(path: Path) -> str | None:
    text = path.read_text(encoding="utf-8")
    match = re.search(r'(?m)^name\s*=\s*"([^"]+)"', text)
    return match.group(1) if match else None


def dependencies(manifest: Path) -> set[str]:
    text = manifest.read_text(encoding="utf-8")
    deps: set[str] = set()
    section = None
    for line in text.splitlines():
        if line.startswith("["):
            section = line.strip()
            continue
        if section == "[dependencies]":
            found = DEP_RE.match(line.strip())
            if found and not found.group(1).startswith("#"):
                deps.add(found.group(1))
    return deps


def check_layers(crates: Path = CRATES, apps: Path = APPS) -> list[str]:
    errors = []
    manifests = list(crates.glob("*/Cargo.toml")) + list(apps.glob("*/Cargo.toml"))
    known = set(LAYERS)
    for manifest in manifests:
        name = crate_name(manifest)
        if name not in LAYERS:
            errors.append(f"unregistered crate {name} at {manifest}")
            continue
        for dep in dependencies(manifest):
            if dep in known:
                errors.extend(layer_violations(name, {dep}))
    return errors


def rel(path: Path, root: Path = ROOT) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return path.as_posix()


def rust_files(crates: Path = CRATES, apps: Path = APPS) -> list[Path]:
    files = []
    for root in (crates, apps):
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            if "/target/" in path.as_posix():
                continue
            files.append(path)
    return files


def check_filesystem(crates: Path = CRATES, apps: Path = APPS, root: Path = ROOT) -> list[str]:
    errors = []
    for path in rust_files(crates, apps):
        shown = rel(path, root)
        if "tests/" in shown or "/tests/" in f"/{shown}":
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        text = text.split("#[cfg(test)]", 1)[0]
        if "/examples/" in f"/{shown}" or "/src/bin/" in f"/{shown}":
            continue
        if FS_RE.search(text) and shown not in FS_ALLOW and rel(path) not in FS_ALLOW:
            errors.append(f"filesystem access outside the allowlist: {shown}")
    return errors


def panic_sites(path: Path, root: Path = ROOT) -> list[str]:
    sites = []
    for number, line in enumerate(
        path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
    ):
        if PANIC_RE.search(line) and "allow" not in line:
            sites.append(f"{rel(path, root)}:{number}:{line.strip()}")
    return sites


def check_panic(
    baseline: Path, crates: Path = CRATES, apps: Path = APPS, root: Path = ROOT
) -> list[str]:
    current = []
    for path in rust_files(crates, apps):
        shown = rel(path, root)
        if "/tests/" in f"/{shown}" or shown.endswith("/tests.rs"):
            continue
        current.extend(panic_sites(path, root))
    expected = [
        line
        for line in baseline.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
    ]
    errors = []
    extra = sorted(set(current) - set(expected))
    # Line numbers drift. Compare the code text after the second colon.
    def payload(site: str) -> str:
        parts = site.split(":", 2)
        return parts[0] + ":" + (parts[2] if len(parts) == 3 else site)

    current_payload = {payload(site) for site in current}
    expected_payload = {payload(site) for site in expected}
    for site in sorted(current_payload - expected_payload):
        errors.append(f"new panic site: {site}")
    return errors


def layer_violations(crate: str, deps: set[str]) -> list[str]:
    allowed = LAYERS.get(crate)
    if allowed is None:
        return [f"unregistered crate {crate}"]
    return [
        f"{crate} depends on {dep}, which its layer does not allow"
        for dep in sorted(deps)
        if dep in LAYERS and dep not in allowed
    ]


def manifest_layer_problems(text: str) -> list[str]:
    """Same allowlist as ``check_layers``, over a manifest fragment."""
    def uncomment(line: str) -> str:
        stripped = line.strip()
        if stripped.startswith("//"):
            stripped = stripped[2:].strip()
        return stripped.lstrip("#").strip()

    name = None
    for line in text.splitlines():
        stripped = uncomment(line)
        if stripped.startswith("name ") or stripped.startswith("name="):
            match = re.search(r'name\s*=\s*"([^"]+)"', stripped)
            if match:
                name = match.group(1)
    deps: set[str] = set()
    section = None
    for line in text.splitlines():
        stripped = uncomment(line)
        if stripped.startswith("["):
            section = stripped
            continue
        if section == "[dependencies]":
            found = DEP_RE.match(stripped)
            if found and not found.group(1).startswith("#"):
                deps.add(found.group(1))
    if name is None or not deps:
        return []
    return layer_violations(name, deps)


def counterexample(path: Path) -> list[str]:
    """Run ``check_layers``, ``check_filesystem`` and ``check_panic`` on a
    throwaway crate. A stubbed guard returns no findings and this command
    then fails.
    """
    import tempfile

    path = path if path.is_absolute() else (ROOT / path)
    source = path.read_text(encoding="utf-8", errors="replace")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        crates = root / "crates"
        apps = root / "apps"
        model = crates / "joyword-model"
        (model / "src").mkdir(parents=True)
        apps.mkdir()
        (model / "Cargo.toml").write_text(
            '[package]\nname = "joyword-model"\n\n[dependencies]\n'
            'joyword-shell = { path = "../joyword-shell" }\n'
        )
        (model / "src" / "lib.rs").write_text(source)
        layers = check_layers(crates, apps)
        files = check_filesystem(crates, apps, root)
        baseline = root / "panic_baseline.txt"
        baseline.write_text("")
        panics = check_panic(baseline, crates, apps, root)
    if not layers or not files or not panics:
        return []
    return layers + files + panics


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] == "--write-baseline":
        sites = []
        for path in rust_files():
            if "/tests/" in f"/{rel(path)}":
                continue
            sites.extend(panic_sites(path))
        baseline = ROOT / "scripts" / "panic_baseline.txt"
        baseline.write_text("\n".join(sites) + "\n", encoding="utf-8")
        print(f"wrote {len(sites)} sites")
        return 0
    if len(sys.argv) > 1 and sys.argv[1] == "--counterexample":
        problems = []
        for arg in sys.argv[2:]:
            problems.extend(counterexample(Path(arg)))
        if not problems:
            print("counterexample was not rejected")
            return 1
        print("\n".join(problems))
        return 0
    errors = []
    errors.extend(check_layers())
    errors.extend(check_filesystem())
    errors.extend(check_panic(ROOT / "scripts" / "panic_baseline.txt"))
    if errors:
        print("\n".join(errors))
        return 1
    print(f"gates ok ({len(list(rust_files()))} rust files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
