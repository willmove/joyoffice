"""Independently reopen candidate XLSX files in a previous supported JoySheet build."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def call(reader, *args):
    return subprocess.run([str(reader), *map(str, args)], check=True, capture_output=True).stdout


def state(reader, path):
    data = json.loads(call(reader, "info", path, "--json"))
    data.pop("path")
    check = json.loads(call(reader, "check", path, "--json", "--max-report", "0"))
    return data, check["totals"]


def verify(old, new, report):
    cases = []
    root = Path(__file__).resolve().parents[2]
    with TemporaryDirectory() as scratch:
        for original in sorted((root / "docs/joysheet/samples").glob("*.xlsx")):
            initial_hash = digest(original)
            dest = Path(scratch) / original.name
            first = state(old, original)
            assert first == state(new, original)
            call(new, "convert", original, dest)
            after = state(old, dest)
            assert after == state(new, dest)
            assert first == after, original.name
            assert digest(original) == initial_hash
            cases.append({"fixture":original.name, "sourceSha256":initial_hash,
                          "sheets":len(after[0]["sheets"]), "checks":after[1]})
        # Known results include arrays, first-error ordering, text coercion and cancellation.
        expected = {
            '=SUM({1,"x",2},TRUE,"3")':7, '=SUM(1E16,1,-1E16)':0,
            '=SUMSQ({2,-3},TRUE)':14, '=PRODUCT({2,0,3})':0, '=PRODUCT({"x","y"})':0,
            '=AVERAGE({1,"x",3})':2, '=MIN({"x","y"})':0, '=MAX(-2,-3)':-2,
            '=PERCENTOF({1,2},{3,3})':0.5, '=PERCENTOF(1,0)':{"error":"#DIV/0!"},
            '=AND("TRUE",{TRUE,"ignore",1})':True, '=OR({0,"TRUE",FALSE})':False,
            '=XOR({TRUE,FALSE,TRUE})':False, '=AND(FALSE,1/0)':{"error":"#DIV/0!"},
            '=OR(TRUE,NA())':{"error":"#N/A"}, '=XOR({"x","y"})':{"error":"#VALUE!"},
        }
        for expression, value in expected.items():
            for reader in (old,new):
                actual = json.loads(call(reader,"eval",expression,"--json"))
                assert actual == value and isinstance(actual,bool) == isinstance(value,bool), (expression,actual,value)
    assert len(cases) >= 3
    report.write_text(json.dumps({"previousBinarySha256":digest(old),"candidateBinarySha256":digest(new),
                                  "formulaExpectationsCheckedInBothVersions":expected,
                                  "previousVersionReopenedCandidateCopies":cases},indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("previous", "candidate", "report"):
        parser.add_argument(name,type=Path)
    args = parser.parse_args()
    verify(args.previous.resolve(),args.candidate.resolve(),args.report)
