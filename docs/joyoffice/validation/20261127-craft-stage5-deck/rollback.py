"""Rehearse package upgrade, downgrade and re-upgrade on isolated document copies."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import zipfile

from audit import ROOT, digest, package_audit, read_json


OPAQUE_PART = "vendor/phase5-opaque.xml"
OPAQUE_XML = b'<vendor xmlns="urn:joydeck:phase5">opaque test content</vendor>'
CHART_SAMPLE = "m9-powerpoint-edited-chart.pptx"


def invoke(exe, profile, *args):
    result = subprocess.run(
        [str(exe), *map(str, args)], cwd=profile, timeout=60,
        env={**os.environ, "APPDATA": str(profile / "roaming"),
             "LOCALAPPDATA": str(profile / "local")},
        capture_output=True, encoding="utf-8",
    )
    assert result.returncode == 0, (exe, args, result.stderr)
    return result.stdout


def zip_parts(file):
    with zipfile.ZipFile(file) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


def extract_package(archive_path, dest):
    with zipfile.ZipFile(archive_path) as archive:
        for name in archive.namelist():
            member = dest / name.split("/", 1)[1]
            assert member.resolve().is_relative_to(dest)
            member.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(name) as src, member.open("xb") as out:
                shutil.copyfileobj(src, out)


def percent_labels(model):
    return [shape["kind"].get("dataLabelPercent", False)
            for slide in model["slides"] for shape in slide["shapes"]
            if shape["kind"]["type"] == "chart"]


def rehearse(work, candidate_archive, baseline):
    evidence = ROOT / "docs/joyoffice/validation/20261124-craft-pilots/verification.json"
    pinned = read_json(evidence)["baseline"]["binarySha256"]
    current, previous, session = (work / name for name in ("candidate", "frozen", "profile"))
    for folder in (current, previous, session):
        folder.mkdir(parents=True)
    package = package_audit(candidate_archive)
    extract_package(candidate_archive, current)
    for name in ("joydeck", "joydeck-cli"):
        executable = f"{name}.exe"
        assert digest((current / executable).read_bytes()) == package["executables"][executable]
        shutil.copy2(baseline / executable, previous / executable)
        assert digest((previous / executable).read_bytes()) == pinned[name]

    fixtures = ROOT / "tests/fixtures/joydeck"
    initial = work / "原件 中文.pptx"
    original = zip_parts(fixtures / "independent.pptx")
    original[OPAQUE_PART] = OPAQUE_XML
    with zipfile.ZipFile(initial, "x", zipfile.ZIP_DEFLATED) as archive:
        for name, data in original.items():
            archive.writestr(name, data)
    initial_sha = digest(initial.read_bytes())
    new_exe, old_exe = (folder / "joydeck-cli.exe" for folder in (current, previous))
    stages = {}
    source = initial
    for label, exe, suffix in (("upgrade", new_exe, "joydeck"), ("downgrade", old_exe, "pptx"),
                               ("reupgrade", new_exe, "joydeck")):
        target = work / f"{label}.{suffix}"
        # Every stage reuses only the isolated profile and the preceding on-disk document.
        previous_info = json.loads(invoke(exe, session, "inspect", source))
        previous_model = json.loads(invoke(exe, session, "content", source))
        shape = previous_info["slides"][0]["objects"][0]["id"]
        text = f"JoyDeck 回退演练 {label} • العربية"
        script = work / f"{label}.json"
        script.write_text(json.dumps([
            {"command": "set-text", "target": {"kind": "shape", "shape": shape},
             "body": {"paragraphs": [{"runs": [{"text": text, "props": {"bold": True}}]}]}},
            {"command": "undo"}, {"command": "redo"},
        ], ensure_ascii=False), encoding="utf-8")
        source_hash = digest(source.read_bytes())
        invoke(exe, session, "edit", source, "--script", script, "--output", target)
        assert digest(source.read_bytes()) == source_hash
        left = json.loads(invoke(old_exe, session, "content", target))
        right = json.loads(invoke(new_exe, session, "content", target))
        assert left == right, label
        info = json.loads(invoke(exe, session, "inspect", target))
        assert info["slides"][0]["objects"][0]["text"] == text
        assert [slide["id"] for slide in previous_model["slides"]] == [
            slide["id"] for slide in right["slides"]
        ]
        assert [[obj["id"] for obj in slide["objects"]] for slide in previous_info["slides"]] == [
            [obj["id"] for obj in slide["objects"]] for slide in info["slides"]
        ]
        parts = zip_parts(target)
        if suffix == "joydeck":
            with zipfile.ZipFile(target) as archive:
                source_pkg = work / f"{label}-source.pptx"
                source_pkg.write_bytes(archive.read("source.package"))
            parts = zip_parts(source_pkg)
        assert parts[OPAQUE_PART] == OPAQUE_XML
        assert all(parts[name] == data for name, data in original.items()
                   if name.startswith(("ppt/media/", "ppt/slideMasters/", "ppt/slideLayouts/")))
        stages[label] = {"file": target.name, "sha256": digest(target.read_bytes()),
                         "slides": len(info["slides"]), "readers": ["frozen", "candidate"]}
        source = target

    chart = work / CHART_SAMPLE
    shutil.copy2(fixtures / CHART_SAMPLE, chart)
    chart_sha = digest(chart.read_bytes())
    original_labels = percent_labels(json.loads(invoke(new_exe, session, "content", chart)))
    assert original_labels == [False, False, False, True, False]
    chart_steps = {}
    for mode in ("preserve", "regenerate"):
        dest = work / f"frozen-chart-{mode}.pptx"
        invoke(old_exe, session, "exchange", chart, "--mode", mode, "--output", dest)
        labels = percent_labels(json.loads(invoke(new_exe, session, "content", dest)))
        chart_steps[mode] = labels
        if mode == "preserve":
            assert zip_parts(chart) == zip_parts(dest)
        assert labels == original_labels
    assert digest(initial.read_bytes()) == initial_sha
    assert digest(chart.read_bytes()) == chart_sha
    return {
        "candidateArchiveSha256": package["archiveSha256"],
        "frozenExecutables": {name: pinned[name] for name in ("joydeck", "joydeck-cli")},
        "inputSha256": initial_sha, "stages": stages,
        "percentageLabels": {"sourceSha256": chart_sha, "candidateSource": original_labels,
                             "frozenWriter": chart_steps},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("candidate_archive", type=Path)
    parser.add_argument("frozen_dir", type=Path)
    parser.add_argument("work", type=Path)
    args = parser.parse_args()
    work = args.work.resolve()
    work.mkdir(parents=True)
    report = rehearse(work, args.candidate_archive.resolve(), args.frozen_dir.resolve())
    (work / "verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8",
                                            newline="\n")
    print("3 isolated save/reopen steps passed with both CLI versions")


if __name__ == "__main__":
    main()
