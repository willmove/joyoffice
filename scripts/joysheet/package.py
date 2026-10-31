"""Build a self-contained Sheet package from a verified, product-scoped snapshot."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import zipfile

from project import ROOT, APPLICATIONS, build, host_target, run

INTEGRATION_DOCS = (
    "docs/joyoffice/JOYSHEET-INTEGRATION.md",
    "docs/joyoffice/JOYSHEET-GATE-AUDIT.md",
    "docs/joyoffice/validation/joysheet-source-inventory-20261031.json",
    "docs/joyoffice/validation/joysheet-migration-map.json",
    "docs/joyoffice/validation/joysheet-test-module-migration.json",
)

# Word already uses this selected normal/build inventory. Sheet reuses it as an
# engineering tool; no application or product runtime dependency is introduced.
sys.path.insert(0, str(ROOT / "scripts/joydeck"))
try:
    import sbom
finally:
    sys.path.pop(0)


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def source_snapshot(target, features):
    snapshot = sbom.snapshot(target, roots=APPLICATIONS, features=features)
    for folder in ("tools/xtask", ".cargo"):
        for path in (ROOT / folder).rglob("*"):
            if path.is_file() and "__pycache__" not in path.parts:
                snapshot[path.relative_to(ROOT).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    snapshot["LICENSE"] = hashlib.sha256((ROOT / "LICENSE").read_bytes()).hexdigest()
    if (ROOT / ".gitattributes").is_file():
        snapshot[".gitattributes"] = hashlib.sha256((ROOT / ".gitattributes").read_bytes()).hexdigest()
    for name in INTEGRATION_DOCS:
        snapshot[name] = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
    return dict(sorted(snapshot.items()))


def filtered_manifest(text, members):
    # Preserve all dependency versions, features, toolchain and profiles. Only
    # remove workspace membership and path entries belonging to other products.
    for key in ("members", "default-members"):
        selected = [p for p in members if key == "members" or p not in
                    {"apps/joysheet", "crates/joysheet-platform", "crates/joysheet-ui-gpui"}]
        text = re.sub(r"(?m)^" + key + r"\s*=\s*\[[\s\S]*?\]",
                      key + " = " + json.dumps(sorted(selected)), text, count=1)
    dependency_specs = tomllib.loads(text)["workspace"].get("dependencies", {})
    lines, section = [], ""
    for line in text.splitlines():
        if line.startswith("["):
            section = line
        if section == "[workspace.dependencies]":
            key = re.match(r"([\w-]+)\s*=", line)
            spec = dependency_specs.get(key[1]) if key else None
            if isinstance(spec, dict) and "path" in spec and spec["path"] not in members:
                continue
        lines.append(line)
    return "\n".join(lines) + "\n"


def source_archive(destination, snapshot, version):
    with tempfile.TemporaryDirectory(prefix="joysheet-source-") as temporary:
        stage = Path(temporary) / "joysheet"
        stage.mkdir()
        for name, expected in snapshot.items():
            source = ROOT / name
            if hashlib.sha256(source.read_bytes()).hexdigest() != expected:
                raise ValueError("Source changed before archive: " + name)
            target = stage / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        original = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        members = [member for pattern in tomllib.loads(original)["workspace"]["members"]
                   for path in ROOT.glob(pattern) if path.is_dir()
                   for member in [path.relative_to(ROOT).as_posix()]
                   if member + "/Cargo.toml" in snapshot]
        (stage / "Cargo.toml").write_text(filtered_manifest(original, members), encoding="utf-8")
        # Prune unused lock records without upgrading any resolved registry package.
        subprocess.run(["cargo", "metadata", "--offline", "--format-version", "1"], cwd=stage,
                       stdout=subprocess.DEVNULL, check=True)
        original_pins = {(p["name"], p["version"], p.get("checksum"))
                         for p in tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))["package"]}
        actual_pins = {(p["name"], p["version"], p.get("checksum"))
                       for p in tomllib.loads((stage / "Cargo.lock").read_text(encoding="utf-8"))["package"]}
        if not actual_pins <= original_pins:
            raise ValueError("Standalone source lock changed dependency pins")
        subprocess.run(["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version", "1"],
                       cwd=stage, stdout=subprocess.DEVNULL, check=True)
        write_json(stage / "MONOREPO-SOURCE.json", {
            "product": "joysheet", "version": version, "originalSourceHashes": snapshot,
            "derivedFiles": {name: hashlib.sha256((stage / name).read_bytes()).hexdigest()
                             for name in ("Cargo.toml", "Cargo.lock")},
            "derivation": "Sheet-only workspace membership and pruned lock; registry pins and checksums preserved",
        })
        (stage / "SOURCE-README.md").write_text(
            "# JoySheet source\n\nThis is a product-scoped workspace derived from JoyOffice. "
            "Run `cargo xtask check joysheet` (on Windows use `pwsh -File scripts/joysheet/dev.ps1 xtask check joysheet`). "
            "The full monorepo CI gates run in the JoyOffice Git repository. "
            "Original and derived inputs are recorded in MONOREPO-SOURCE.json.\n", encoding="utf-8")
        with zipfile.ZipFile(destination, "x", zipfile.ZIP_DEFLATED, strict_timestamps=False) as archive:
            for path in sorted(stage.rglob("*")):
                if path.is_file():
                    archive.write(path, "joysheet/" + path.relative_to(stage).as_posix())


def package(profile, validation, output=None):
    version = tomllib.loads((ROOT / "apps/joysheet/Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    cli_version = tomllib.loads((ROOT / "apps/joysheet-cli/Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    if version != cli_version:
        raise ValueError("Sheet GUI/CLI versions differ")
    target = host_target()
    features = ("joysheet/automation",) if validation else ()
    before = source_snapshot(target, features)
    artifacts = build(profile, validation)
    sbom.require_unchanged_sources(before, source_snapshot(target, features))
    system = {"Darwin": "macos", "Windows": "windows", "Linux": "linux"}[platform.system()]
    architecture = target.split("-", 1)[0]
    label = f"joysheet-{version}-{system}-{architecture}-{profile}" + ("-validation" if validation else "")
    destination = Path(output).resolve() if output else ROOT / "dist/joysheet" / label
    destination.parent.mkdir(parents=True, exist_ok=True)
    # Refuse stale output rather than mixing a previously verified payload with a rebuild.
    destination.mkdir()
    for name, artifact in artifacts.items():
        shutil.copy2(artifact, destination / artifact.name)
    for name in ("LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "ATTRIBUTION.md"):
        shutil.copy2(ROOT / "docs/joysheet" / name, destination / name)
    (destination / "README.md").write_text(
        f"# JoySheet {version} preview\n\nRun joysheet (joysheet.exe on Windows), or use the sample launchers. "
        "See [product documentation](docs/joysheet/README.md) and [known limits](docs/joysheet/STATUS.md). "
        "This development package does not imply completed M8, native-platform or signing acceptance.\n", encoding="utf-8")
    shutil.copytree(ROOT / "docs/joysheet", destination / "docs/joysheet")
    for name in INTEGRATION_DOCS:
        target_doc = destination / name
        target_doc.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / name, target_doc)
    shutil.copytree(ROOT / "assets/joysheet", destination / "assets/joysheet")
    shutil.copytree(ROOT / "scripts/joysheet", destination / "scripts/joysheet",
                    ignore=shutil.ignore_patterns("__pycache__", "tests"))
    inventory = sbom.generate(destination / "SBOM.spdx.json", target, profile, roots=APPLICATIONS, features=features,
                              require_license_texts=True)
    manifest = {
        "product": "joysheet", "version": version, "stage": "M8-preview6-monorepo", "milestoneComplete": False,
        "profile": profile, "target": target, "automation": validation, "publisherSigned": False,
        "sourceHashes": before, "dependencyInventory": inventory,
        "binaries": {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in artifacts.values()},
        "nativeAcceptance": "not implied by packaging; see migration verification report",
    }
    write_json(destination / "BUILD.json", manifest)
    write_json(destination / "SOURCE-HASHES.json", before)
    write_json(destination / "BUILD-INFO.json", {
        "version": version, "target": target, "profile": profile, "packageRoots": list(APPLICATIONS),
        "features": list(features), "sourceSnapshotVerified": True,
        "officialSignatureVerified": False, "cargoLockSha256": inventory["lockSha256"],
        "sourceGraphSha256": hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(),
    })
    if platform.system() == "Windows":
        for name, sample in [("Open sales sample.cmd", None), ("Open Windows quality.cmd", "Windows-quality.xlsx"),
                             ("Open data workflow.cmd", "M5-workflow.xlsx"), ("Open advanced workflow.cmd", "M7-workflow.xlsx")]:
            argument = "--sample sales" if sample is None else f'"docs\\joysheet\\samples\\{sample}"'
            (destination / name).write_text(f'@echo off\r\ncd /d "%~dp0"\r\njoysheet.exe {argument}\r\n', encoding="utf-8")
        if validation:
            for name, args in [("Run dense sheet benchmark.cmd", "-StressCases"),
                               ("Run performance benchmark.cmd", "-Performance -FormulaCases")]:
                (destination / name).write_text('@echo off\r\ncd /d "%~dp0"\r\npwsh.exe -NoProfile -File '
                    f'"scripts\\joysheet\\desktop-smoke.ps1" -BinaryDirectory "." -Stage M1 {args} %*\r\npause\r\n', encoding="utf-8")
    from native import bundle
    bundle(destination, version)
    source_archive(destination.parent / (destination.name + "-source.zip"), before, version)
    sbom.require_unchanged_sources(before, source_snapshot(target, features))
    checksums = {p.relative_to(destination).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                 for p in sorted(destination.rglob("*")) if p.is_file()}
    write_json(destination / "PAYLOAD-SHA256.json", checksums)
    archive_path = destination.parent / (destination.name + ".zip")
    with zipfile.ZipFile(archive_path, "x", zipfile.ZIP_DEFLATED, strict_timestamps=False) as archive:
        for path in sorted(destination.rglob("*")):
            if path.is_file():
                archive.write(path, destination.name + "/" + path.relative_to(destination).as_posix())
    archive_path.with_name(archive_path.name + ".sha256").write_text(
        hashlib.sha256(archive_path.read_bytes()).hexdigest() + "  " + archive_path.name + "\n", encoding="ascii")
    print(destination)
    return destination


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=["debug", "release"], default="release")
    parser.add_argument("--validation", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--checkpoint-only", action="store_true")
    args = parser.parse_args(argv)
    if args.checkpoint_only:
        output = ROOT / "artifacts/joysheet/source-checkpoint.json"
        output.parent.mkdir(parents=True, exist_ok=True)
        write_json(output, source_snapshot(host_target(), ("joysheet/automation",) if args.validation else ()))
        print(output)
    else:
        package(args.profile, args.validation, args.output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print("error: " + str(error), file=sys.stderr)
        sys.exit(1)
