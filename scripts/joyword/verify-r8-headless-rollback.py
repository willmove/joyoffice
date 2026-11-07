#!/usr/bin/env python3
"""Verify a Linux CLI-only R8 upgrade/rollback in a new isolated directory.

Inputs are trusted, runnable CLI binaries, not release packages. This does not
validate GUI startup, installers, file associations, signatures or licensing.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

SPEC = importlib.util.spec_from_file_location(
    "word_rollback", Path(__file__).with_name("verify-r8-rollback.py"))
rollback = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rollback)
require = rollback.require
digest = rollback.digest
candidate_in_place = rollback.candidate_in_place


def stage(binary, destination, fingerprint):
    destination.mkdir()
    target = destination / "joyword-cli"
    shutil.copy2(binary, target)
    require(digest(target) == fingerprint, "Staged CLI differs from its input")
    require(os.access(target, os.X_OK), "Staged CLI is not executable")


def invoke(directory, env, *args):
    result = subprocess.run([directory / "joyword-cli", *args], cwd=directory,
                            env=env, capture_output=True, timeout=90, check=False)
    require(result.returncode == 0,
            f"CLI validation failed ({result.returncode}): {args[0]}")
    return json.loads(result.stdout)


def inspect(directory, env, path):
    return invoke(directory, env, "inspect", str(path), "--compatibility")


def verify(previous, candidate, fixtures, work):
    require(sys.platform.startswith("linux"), "This headless exercise requires Linux")
    previous, candidate, fixtures, work = [Path(p).resolve() for p in
                                          (previous, candidate, fixtures, work)]
    require(previous.is_file() and candidate.is_file(), "Both CLI inputs must be regular files")
    require(os.access(previous, os.X_OK) and os.access(candidate, os.X_OK),
            "Both CLI inputs must be executable")
    old_hash, new_hash = digest(previous), digest(candidate)
    require(old_hash != new_hash, "Candidate and previous CLI must differ")
    require(not work.exists(), "Output directory must be new")
    require(all((fixtures / name).is_file() for name in ("source.docx", "snapshot.docx"))
            and (fixtures / "rich").is_dir(), "Missing source/snapshot/rich recovery fixtures")
    fixture_hashes = {p: digest(p) for p in fixtures.rglob("*") if p.is_file()}
    work.mkdir(parents=True)
    install, previous_dir = work / "install", work / "previous"
    stage(previous, install, old_hash)
    profile = work / "用户 配置"
    profile.mkdir()
    preferences = profile / "preferences.conf"
    preferences.write_text("ui.locale=en\nautosave.minutes=0\n", encoding="utf-8")
    recovery = profile / "recovery"
    shutil.copytree(fixtures / "rich", recovery)
    source = work / "源 文档.docx"
    shutil.copy2(fixtures / "source.docx", source)
    originals = {p: digest(p) for p in (source, preferences, *recovery.rglob("*")) if p.is_file()}
    env = {**os.environ, "JOYWORD_CONFIG_DIR": str(profile), "JOYWORD_RECOVERY_DIR": str(recovery)}
    env.pop("JOYWORD_CONTROL_KEY", None)
    env.pop("JOYWORD_PROFILE_DIR", None)
    before = inspect(install, env, source)

    # Promotion fails after the old installation is moved aside.
    try:
        with candidate_in_place(install, work / "missing-stage", previous_dir, work / "unused"):
            raise AssertionError("Missing stage was promoted")
    except FileNotFoundError:
        require(digest(install / "joyword-cli") == old_hash and not previous_dir.exists(),
                "Failed promotion did not restore the previous CLI")

    # An actual candidate process fails validation after promotion, then rolls back.
    failed_stage = work / "failed-stage"
    stage(candidate, failed_stage, new_hash)
    try:
        with candidate_in_place(install, failed_stage, previous_dir, work / "rejected-candidate"):
            invoke(install, env, "inspect", str(work / "nonexistent.docx"), "--compatibility")
            raise AssertionError("Candidate accepted the nonexistent verification input")
    except ValueError as error:
        require(str(error).startswith("CLI validation failed"), "Unexpected validation failure")
        require(digest(install / "joyword-cli") == old_hash and not previous_dir.exists(),
                "Validation failure did not restore the previous CLI")
        require(inspect(install, env, source) == before,
                "Previous CLI failed readback after rejected candidate")

    staged = work / "staged"
    stage(candidate, staged, new_hash)
    restored = work / "恢复 副本.docx"
    with candidate_in_place(install, staged, previous_dir, work / "retired-candidate"):
        require(digest(install / "joyword-cli") == new_hash, "Candidate was not promoted")
        require(inspect(install, env, source) == before, "Candidate compatibility report differs")
        invoke(install, env, "recover", str(recovery), "--save", str(restored))
        require(digest(restored) == digest(fixtures / "snapshot.docx"),
                "Recovered snapshot is not byte-identical")
    require(digest(install / "joyword-cli") == old_hash and not previous_dir.exists(),
            "Successful validation did not roll back to the old CLI")
    require(inspect(install, env, source) == before, "Rolled-back source report differs")
    require(inspect(install, env, restored) == inspect(install, env, fixtures / "snapshot.docx"),
            "Old CLI cannot read back the recovered snapshot")
    require(all(p.is_file() and digest(p) == value for p, value in originals.items()),
            "An isolated original document, preference or recovery file changed")
    require(all(p.is_file() and digest(p) == value for p, value in fixture_hashes.items()),
            "An original fixture changed")
    require(digest(previous) == old_hash and digest(candidate) == new_hash,
            "An input CLI changed")
    report = {
        "scope": "linux-cli-only", "previous_cli_sha256": old_hash,
        "candidate_cli_sha256": new_hash, "missing_stage_restored_previous": True,
        "candidate_process_failure_restored_previous": True,
        "successful_promotion_and_rollback": True, "exact_v3_snapshot": True,
        "old_cli_reads_source_and_recovery": True,
        "isolated_originals_unchanged": len(originals), "fixtures_unchanged": len(fixture_hashes),
        "gui_verified": False, "installer_verified": False,
        "file_association_verified": False, "release_package_verified": False,
    }
    (work / "verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("previous", "candidate", "fixtures", "out"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.previous, args.candidate, args.fixtures, args.out)))
