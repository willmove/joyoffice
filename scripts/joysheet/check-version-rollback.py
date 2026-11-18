"""Independently reopen candidate XLSX files in a previous supported JoySheet build."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
from xml.etree import ElementTree
from zipfile import ZipFile


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


def iterative_roundtrips(old, new, directory):
    fixtures = [
        ("count", True, 3, .001, ["=A1+1", "=A1*2"], ["3", "6"], ["6", "12"]),
        ("change", True, 100, 1., ["=(A2+10)/2", "=A1"], ["9.6875", "9.375"], ["9.84375", "9.6875"]),
        ("textAndError", True, 10, .001, ['=IF(ISTEXT(A1),A1&"","go")', "=A2+1/0"],
         ["go", "#DIV/0!"], ["go", "#DIV/0!"]),
        ("disabled", False, 100, .001, ["=A1+1", "=A1*2"], ["#CIRC!", "#CIRC!"], ["#CIRC!", "#CIRC!"]),
    ]
    result = []
    for name, enabled, count, change, formulas, saved, reopened in fixtures:
        settings = {"iterative": enabled, "maxIterations": count, "maxChange": change}
        commands = ["--cmd", "formulas.calculationOptions=" + json.dumps(settings)]
        for index, formula in enumerate(formulas, 1):
            commands.extend(["--cmd", "cell.set=" + json.dumps({"cell": f"A{index}", "input": formula})])
        for version, writer in [("previous", old), ("candidate", new)]:
            workbook = directory / f"{version}-{name}.xlsx"
            rows = call(writer, "run", *commands, "--out", workbook, "--print", "A1:A2", "--csv", "--quiet")
            assert rows.decode().splitlines() == saved, (name, version, rows)
            with ZipFile(workbook) as package:
                props = ElementTree.fromstring(package.read("xl/workbook.xml")).find("{*}calcPr")
                data = ElementTree.fromstring(package.read("xl/worksheets/sheet1.xml"))
                assert [f.text for f in data.findall(".//{*}f")] == [f[1:] for f in formulas]
                assert (props.get("iterate") == "1") == enabled
                if enabled:
                    assert int(props.get("iterateCount")) == count
                    assert float(props.get("iterateDelta")) == change
            for reader in [old, new]:
                actual = call(reader, "cat", workbook, "--range", "A1:A2", "--csv").decode().splitlines()
                assert actual == reopened, (name, version, actual)
        result.append({"case": name, "options": settings, "formulas": formulas,
                       "beforeSave": saved, "afterReopen": reopened, "writerReaderCombinations": 4})
    return result


def lookup_roundtrips(old, new, folder):
    inputs = {"A1": "1", "B1": "10", "A2": "5", "B2": "20", "A3": "5", "B3": "30",
              "A4": "9", "B4": "40", "A5": "foooo", "B5": "50"}
    formulas = [
        ("=VLOOKUP(5,A1:B4,2,TRUE)", "30"),
        ("=HLOOKUP(5,A2:B3,2,FALSE)", "5"),
        ("=LOOKUP(6,A1:A4,B1:B4)", "30"),
        ("=MATCH(5,A1:A4,0)", "2"),
        ("=XMATCH(6,A1:A4,-1,-1)", "3"),
        ("=XLOOKUP(5,A1:A4,B1:B4,0,0,-1)", "30"),
        ('=XLOOKUP("fo*",A1:A5,B1:B5,0,2)', "50"),
        ("=XMATCH(0,A1:A4,-1,1)", "#N/A"),
    ]
    inputs.update({f"D{i}": f for i, (f, _) in enumerate(formulas, 1)})
    options = [value for key, expr in inputs.items() for value in
               ("--cmd", "cell.set=" + json.dumps({"cell": key, "input": expr}))]
    addresses = f"D1:D{len(formulas)}"
    expected = [value for _, value in formulas]
    for label, writer in [("old", old), ("new", new)]:
        path = folder / f"lookups-{label}.xlsx"
        csv = call(writer, "run", *options, "--out", path, "--print", addresses, "--csv", "--quiet")
        assert csv.decode().splitlines() == expected
        with ZipFile(path) as archive:
            sheet = ElementTree.fromstring(archive.read("xl/worksheets/sheet1.xml"))
            written = [node.text.removeprefix("_xlfn.") for node in sheet.findall(".//{*}f")]
            assert written == [f[1:] for f, _ in formulas]
        for reader in (old, new):
            actual = call(reader, "cat", path, "--range", addresses, "--csv").decode().splitlines()
            assert actual == expected
    return {"formulas": dict(formulas), "writerReaderCombinations": 4}


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
        iterative = iterative_roundtrips(old, new, Path(scratch))
        lookups = lookup_roundtrips(old, new, Path(scratch))
        # Known results include arrays, first-error ordering, text coercion and cancellation.
        expected = {
            '=SUM({1,"x",2},TRUE,"3")':7, '=SUM(1E16,1,-1E16)':0,
            '=SUMSQ({2,-3},TRUE)':14, '=PRODUCT({2,0,3})':0, '=PRODUCT({"x","y"})':0,
            '=AVERAGE({1,"x",3})':2, '=MIN({"x","y"})':0, '=MAX(-2,-3)':-2,
            '=PERCENTOF({1,2},{3,3})':0.5, '=PERCENTOF(1,0)':{"error":"#DIV/0!"},
            '=AND("TRUE",{TRUE,"ignore",1})':True, '=OR({0,"TRUE",FALSE})':False,
            '=XOR({TRUE,FALSE,TRUE})':False, '=AND(FALSE,1/0)':{"error":"#DIV/0!"},
            '=OR(TRUE,NA())':{"error":"#N/A"}, '=XOR({"x","y"})':{"error":"#VALUE!"},
            '=MEDIAN(9,1,7,3)':5, '=MEDIAN(9,1,7)':7,
            '=MODE(7,5,7,5)':7, '=MODE.SNGL(2,3,4)':{"error":"#N/A"},
            '=MODE.MULT({4,1,4,9,1,9,2})':[[4],[1],[9]],
            '=LARGE({7,3,3,1},2)':3, '=SMALL({8,1,3},1.1)':3,
            '=FREQUENCY({1,2,3,4},{3,1,3})':[[2],[1],[0],[1]],
            '=MATCH("a*",{"Beta","Abc","Ax"},0)':2, '=MATCH(2,{1,2,2,4},1)':3,
            '=VLOOKUP(5,{1,10;5,20;5,30;9,40},2,TRUE)':30,
            '=HLOOKUP("A*",{"Cat","Abc";1,2},2,FALSE)':2,
            '=LOOKUP(4,{1,3,5;11,13,15})':13,
            '=XMATCH(3,{8,2,2,6},-1,1)':2, '=XMATCH(3,{8,2,2,6},-1,-1)':3,
            '=XMATCH(4,{1,2,2,6,9},1,2)':4, '=XMATCH(4,{9,6,2,2,1},1,-2)':2,
            '=XLOOKUP(3,{7,1,2,2},{"a","b","c","d"},"missing",-1,-1)':"d",
            '=XLOOKUP(5,{1;5;9},{1,2;3,4;5,6})':[[3,4]],
        }
        for expression, value in expected.items():
            for reader in (old,new):
                actual = json.loads(call(reader,"eval",expression,"--json"))
                assert actual == value and isinstance(actual,bool) == isinstance(value,bool), (expression,actual,value)
    assert len(cases) >= 3
    report.write_text(json.dumps({"previousBinarySha256":digest(old),"candidateBinarySha256":digest(new),
                                  "formulaExpectationsCheckedInBothVersions":expected,
                                  "previousVersionReopenedCandidateCopies":cases,
                                  "iterativeXlsxRoundtrips":iterative,"lookupXlsxRoundtrips":lookups},indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("previous", "candidate", "report"):
        parser.add_argument(name,type=Path)
    args = parser.parse_args()
    verify(args.previous.resolve(),args.candidate.resolve(),args.report)
