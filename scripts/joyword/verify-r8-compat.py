#!/usr/bin/env python3
"""Verify the legacy CLI and recovery on an unpacked JoyWord build."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import xml.etree.ElementTree as ET
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[2]
WORD = {"w": "http://schemas.openxmlformats.org/wordprocessingml/2006/main"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(cli, *args, cwd=None, expected=0):
    result = subprocess.run([cli, *args], cwd=cwd, capture_output=True, timeout=90, check=False)
    require(result.returncode == expected, f"{args}: {result.returncode} != {expected}: {result.stderr!r}")
    return result


def inspect(cli, path, *flags):
    return json.loads(run(cli, "inspect", path, *flags).stdout)


def verify(cli, fixtures, work, baseline):
    original = {p.relative_to(fixtures).as_posix(): sha(p) for p in fixtures.rglob("*") if p.is_file()}
    comparison = {}
    paths = sorted((ROOT / "tests/fixtures/docx").glob("*.docx"))
    paths += sorted((ROOT / "tests/fixtures/feature").glob("*.docx"))
    paths += sorted((ROOT / "assets/samples").glob("*.docx"))
    for path in paths:
        before = sha(path)
        report = inspect(cli, path, "--compatibility")
        text = run(cli, "text", "--compatibility", path)
        require(text.stdout.decode("utf-8") == report["text"], f"text mismatch: {path}")
        require(json.loads(text.stderr) == {k: v for k, v in report.items() if k != "text"},
                f"metadata mismatch: {path}")
        require(report["schema_version"] == 1 and sha(path) == before, f"schema/source: {path}")
        if baseline:
            old = inspect(baseline, path, "--compatibility")
            comparison[path.name] = sorted(k for k in old if old[k] != report[k])
            require(old["text"] == report["text"], f"legacy projection differs: {path}")

    literal = work / "--max-text-bytes"
    shutil.copyfile(fixtures / "source.docx", literal)
    for limit, text, status in [(0, "", 3), (2, "", 3), (3, "合", 3), (6, "合作", 3), (1048576, None, 0)]:
        args = ("--max-text-bytes", str(limit))
        before = run(cli, "inspect", *args, "--", literal.name, cwd=work, expected=status)
        after = run(cli, "inspect", str(literal), *args, expected=status)
        require(before.stdout == after.stdout, "option order or literal path changed report")
        report = json.loads(before.stdout)
        require(report["truncated"] == (status == 3), "truncation flag")
        require(text is None or report["text"] == text, f"invalid UTF-8 prefix at limit {limit}")
    for args in [
        ("inspect", "--max-text-bytes", "1048577", literal),
        ("text", "--max-text-bytes"),
        ("text", "--compatibility", "--compatibility", literal),
        ("inspect", "--not-an-option"),
        ("inspect", work / "missing.docx"),
    ]:
        result = run(cli, *args, expected=2)
        require(not result.stdout and result.stderr, f"missing error contract: {args}")

    cases = {
        "single/recovery.jwr": "单槽😀", "dual/recovery-a.jwr": "定稿😀", "rich": "旧版恢复😀",
        "fallback/recovery-b.jwr": "初稿", "source-missing": "定稿😀",
    }
    recovery = {}
    for case, marker in cases.items():
        name = case.split("/")[0]
        output = work / f"{name}.docx"
        result = run(cli, "recover", fixtures / case, "--save", output)
        require(marker in json.loads(result.stdout)["text"], f"wrong checkpoint: {case}")
        require(marker in inspect(cli, output)["text"], f"reopen lost recovery text: {case}")
        session = json.loads(run(cli, "inspect", "--session", "--", output).stdout)
        require(marker in session["text"] and session["pages"], f"R8 session route: {case}")
        after = sha(output)
        refusal = run(cli, "recover", fixtures / case, "--save", output, expected=1)
        require(refusal.stderr and sha(output) == after, f"no-clobber: {case}")
        if name == "fallback":
            require(b"damaged" in result.stderr, "fallback warning missing")
        if name == "rich":
            require(after == original["snapshot.docx"], "rich package did not survive byte for byte")
            with ZipFile(output) as package:
                root = ET.fromstring(package.read("word/document.xml"))
                require(root.find(".//w:tbl", WORD) is not None, "recovered table missing")
                require(root.find(".//w:b", WORD) is not None, "recovered run formatting missing")
        recovery[name] = after

    failures = {
        "broken": b"checksum", "discarded": b"discarded", "mismatch": b"body text mismatch",
        "conflict": b"same generation",
    }
    for case, diagnostic in failures.items():
        output = work / f"{case}.docx"
        failure = run(cli, "recover", fixtures / case, "--save", output, expected=1)
        require(diagnostic in failure.stderr and not output.exists(), f"failure path: {case}")
    run(cli, "recover", fixtures / "dual", "--save", fixtures / "source.docx", expected=1)
    require(original == {p.relative_to(fixtures).as_posix(): sha(p)
                         for p in fixtures.rglob("*") if p.is_file()},
            "legacy logs, locks or source DOCX changed")
    return {"cli": sha(cli), "legacy_cli_differences": comparison,
            "recovered_docx": recovery, "fixture_hashes": original,
            "refusals": [*failures, "overwrite"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("cli", type=Path)
    parser.add_argument("fixtures", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True)
    print(json.dumps(verify(args.cli.resolve(), args.fixtures.resolve(), args.output.resolve(),
                            args.baseline.resolve() if args.baseline else None),
                     indent=2, ensure_ascii=True))


if __name__ == "__main__":
    main()
