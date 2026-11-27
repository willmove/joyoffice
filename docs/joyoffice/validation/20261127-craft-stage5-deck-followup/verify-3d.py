"""Replay preserving save and rollback on a PowerPoint-authored GLB presentation."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import zipfile


def sha(data):
    return hashlib.sha256(data).hexdigest()


def parts(file):
    with zipfile.ZipFile(file) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


def invoke(exe, output, *args, expected_status=0):
    result = subprocess.run(
        [str(exe), *map(str, args)], cwd=output, timeout=90, encoding="utf-8",
        env={**os.environ, "APPDATA": str(output / "roaming"), "LOCALAPPDATA": str(output / "local")},
        capture_output=True,
    )
    assert result.returncode == expected_status, (args, result.returncode, result.stderr)
    return result


def shape_kinds(exe, output, file):
    model = json.loads(invoke(exe, output, "content", file).stdout)
    return [[shape["kind"]["type"] for shape in slide["shapes"]] for slide in model["slides"]]


def main():
    args = argparse.ArgumentParser(description=__doc__)
    for name in ("frozen", "candidate", "source", "work"):
        args.add_argument(name, type=Path)
    options = args.parse_args()
    frozen = options.frozen.resolve()
    candidate = options.candidate.resolve()
    source = options.source.resolve()
    work = options.work.resolve()
    work.mkdir(parents=True)
    original = parts(source)
    original_sha = sha(source.read_bytes())
    [glb] = [name for name in original if name.lower().endswith(".glb")]
    assert shape_kinds(candidate, work, source) == [["model3d"]]
    assert shape_kinds(frozen, work, source) == [["picture"]]

    preserved = work / "candidate-preserve.pptx"
    rollback = work / "frozen-preserve.pptx"
    regenerated = work / "frozen-regenerate.pptx"
    for exe, inp, mode, output in (
        (candidate, source, "preserve", preserved),
        (frozen, preserved, "preserve", rollback),
        (frozen, preserved, "regenerate", regenerated),
    ):
        invoke(exe, work, "exchange", inp, "--mode", mode, "--output", output)
    for copy in (preserved, rollback):
        assert parts(copy) == original
        assert shape_kinds(candidate, work, copy) == [["model3d"]]
        assert shape_kinds(frozen, work, copy) == [["picture"]]
    assert all(not name.lower().endswith(".glb") for name in parts(regenerated))
    assert shape_kinds(candidate, work, regenerated) == [["picture"]]

    edits = work / "duplicate.json"
    edits.write_text('[{"command":"duplicate-slide"}]\n',
                     encoding="utf-8", newline="\n")
    duplicated = work / "candidate-duplicate.pptx"
    refused = invoke(candidate, work, "edit", source, "--script", edits, "--output", duplicated,
                     expected_status=1)
    assert "3D slides remain intact" in refused.stderr and not duplicated.exists()
    assert sha(source.read_bytes()) == original_sha
    result = {
        "sourceSha256": original_sha,
        "frozenCliSha256": sha(frozen.read_bytes()),
        "candidateCliSha256": sha(candidate.read_bytes()),
        "glbPart": glb,
        "glbSha256": sha(original[glb]),
        "unchangedPreservedMembers": len(original),
        "regeneratedFrozenModel": "picture",
        "regeneratedContainsGlb": False,
        "duplicateRejectedWithoutOutput": True,
    }
    (work / "verification.json").write_text(json.dumps(result, indent=2) + "\n",
                                           encoding="utf-8", newline="\n")
    print("3-D preserve, rollback, regenerate and duplicate refusal verified")


if __name__ == "__main__":
    main()
