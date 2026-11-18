"""Verify private XLSX originals and derived numeric formula samples; publish counts only."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import tempfile
import time
from xml.dom import minidom
import zipfile

NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"


def column_number(name):
    result = 0
    for char in name:
        result = result * 26 + ord(char) - ord("A") + 1
    return result


def column_name(number):
    result = ""
    while number:
        number, index = divmod(number - 1, 26)
        result = chr(ord("A") + index) + result
    return result


def derive(source, target):
    """Add a numeric precedent, a formula at each populated numeric row, and an ordered SUM."""
    def nodes(element, name):
        return element.getElementsByTagNameNS(NS, name)

    with zipfile.ZipFile(source) as original:
        candidates = []
        original_formulas = 0
        for member in original.namelist():
            if not re.fullmatch(r"xl/worksheets/[^/]+\.xml", member):
                continue
            sheet = minidom.parseString(original.read(member))
            original_formulas += len(nodes(sheet, "f"))
            columns = {}
            max_col = max_row = 0
            for cell in nodes(sheet, "c"):
                ref = re.fullmatch(r"([A-Z]+)([1-9][0-9]*)", cell.getAttribute("r"))
                if not ref:
                    continue
                col, row = ref[1], int(ref[2])
                max_col, max_row = max(max_col, column_number(col)), max(max_row, row)
                if cell.getAttribute("t") not in ("", "n") or nodes(cell, "f"):
                    continue
                values = nodes(cell, "v")
                if values and values[0].firstChild:
                    value = float(values[0].firstChild.data)
                    if math.isfinite(value):
                        columns.setdefault(col, []).append((row, value, cell.parentNode))
            if columns and max_col < 16384 and max_row < 1048576:
                col = max(columns, key=lambda name: len(columns[name]))
                candidates.append((len(columns[col]), member, sheet, col, columns[col], max_col, max_row))
        if not candidates:
            return {"originalFormulaCells": original_formulas, "derivedFormulaCells": 0}
        count, member, sheet, src_col, rows, last_col, last_row = max(candidates, key=lambda item: item[0])
        out_col = column_name(last_col + 1)
        prefix = (sheet.documentElement.prefix + ":") if sheet.documentElement.prefix else ""

        def element(kind):
            return sheet.createElementNS(NS, prefix + kind)

        def add_formula(row, number, expression, value):
            cell = element("c")
            cell.setAttribute("r", out_col + str(number))
            for name, text in (("f", expression), ("v", repr(value))):
                part = element(name)
                part.appendChild(sheet.createTextNode(text))
                cell.appendChild(part)
            row.appendChild(cell)

        total = 0.
        for row, value, container in sorted(rows, key=lambda entry: entry[0]):
            add_formula(container, row, f"{src_col}{row}+1", value + 1)
            total += value + 1
        summary = element("row")
        summary.setAttribute("r", str(last_row + 1))
        nodes(sheet, "sheetData")[0].appendChild(summary)
        add_formula(summary, last_row + 1, f"SUM({out_col}1:{out_col}{last_row})", total)
        for extent in nodes(sheet, "dimension"):
            extent.setAttribute("ref", f"A1:{out_col}{last_row + 1}")
        with zipfile.ZipFile(target, "w") as output:
            for entry in original.infolist():
                output.writestr(entry, sheet.toxml(encoding="utf-8") if entry.filename == member else original.read(entry))
    return {"originalFormulaCells": original_formulas, "derivedFormulaCells": count + 1}


def oracle(binary, doc):
    p = subprocess.run([binary, "check", doc, "--json", "--max-report", "0"], capture_output=True)
    if p.returncode not in (0, 1):
        raise RuntimeError("the checker could not read the workbook")
    report = json.loads(p.stdout)
    return {"exit": p.returncode, "counts": report["totals"]}


def timed(binary, doc):
    start = time.perf_counter()
    args = [binary, "run", "--in", doc, "--quiet"] + ["--cmd", "formulas.calculateNow={}"] * 12
    subprocess.run(args, capture_output=True, check=True)
    return round(1000 * (time.perf_counter() - start), 3)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for parameter in ("baseline", "candidate", "inputs", "report"):
        parser.add_argument(parameter, type=Path)
    args = parser.parse_args()
    configs = [("baseline", args.baseline.resolve()), ("candidate", args.candidate.resolve())]
    cases = []
    for source in sorted(args.inputs.rglob("*")):
        if source.suffix.lower() != ".xlsx" or source.is_symlink() or source.name.startswith(("~$", ".")):
            continue
        fingerprint = hashlib.sha256(source.read_bytes()).hexdigest()
        with tempfile.TemporaryDirectory() as tmp:
            derived = Path(tmp) / "derived.xlsx"
            case = {"sourceSha256": fingerprint, **derive(source, derived)}
            for label, path in (("original", source), ("derived", derived)):
                if not path.exists():
                    continue
                checks = {key: oracle(binary, path) for key, binary in configs}
                if checks["baseline"] != checks["candidate"]:
                    raise RuntimeError(f"{fingerprint} {label}: calc or round-trip difference between the two builds")
                case[label] = checks
            if derived.is_file():
                samples = {key: [] for key, _ in configs}
                for _, binary in configs:
                    timed(binary, derived)
                for pair in range(6):
                    for key, binary in (reversed(configs) if pair % 2 else configs):
                        samples[key].append(timed(binary, derived))
                case["openAnd12FullRecalcsMs"] = samples
            assert hashlib.sha256(source.read_bytes()).hexdigest() == fingerprint
            cases.append(case)
    if not cases:
        raise ValueError("no XLSX samples")
    args.report.write_text(json.dumps(sorted(cases, key=lambda case: case["sourceSha256"]), indent=2) + "\n")


if __name__ == "__main__":
    main()
