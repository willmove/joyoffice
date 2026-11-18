"""Record the native Excel for Mac result for the synthetic JoySheet broadcast board."""

import argparse
import hashlib
import json
import plistlib
import re
import runpy
import secrets
import subprocess
import time
from pathlib import Path
from zipfile import BadZipFile

import xlsxwriter

board = runpy.run_path(str(Path(__file__).with_name("check-broadcast.py")))


def applescript(source):
    return subprocess.run(
        ["osascript", "-"], input=source, text=True, check=True, capture_output=True, timeout=10
    ).stdout.strip()


def wait_for(read, label):
    expires = time.monotonic() + 45
    while time.monotonic() < expires:
        try:
            value = read()
            if value:
                return value
        except (FileNotFoundError, BadZipFile, subprocess.SubprocessError):
            pass
        time.sleep(0.25)
    raise RuntimeError(f"Excel did not {label}")


def record(excel, folder, inputs, formulas):
    name = "JoyOffice-broadcast-" + secrets.token_hex(6)
    path = folder / (name + ".xlsx")
    with xlsxwriter.Workbook(path, {"use_future_functions": True}) as workbook:
        sheet = workbook.add_worksheet("Probe")
        for address, value in inputs.items():
            sheet.write(address, -1 if address == "A1" else value)
        sheet.set_selection("A1")
        for address, expression in formulas.items():
            # All LAMBDA parameters in the fixed probes are named x or y.
            if "LAMBDA(" in expression:
                expression = re.sub(r"\b([xy])\b", r"_xlpm.\1", expression)
            sheet.write_dynamic_array_formula(address, expression)
    original = hashlib.sha256(path.read_bytes()).hexdigest()
    subprocess.run(["open", "-a", str(excel), str(path)], check=True)
    process = 'application process "Microsoft Excel"'

    def front():
        return applescript(
            f'tell application "System Events" to return name of front window of {process}'
        )

    def keys(lines):
        applescript(
            f'tell application "System Events" to tell {process}\n'
            f'if name of front window is not "{name}" and name of front window is not "{name}.xlsx" then error "Wrong workbook"\n'
            + lines
            + "\nend tell"
        )

    wait_for(lambda: front() in (name, path.name), "open the new synthetic workbook")
    offset = inputs["A1"]
    keys(f'keystroke "{offset}"\nkey code 36\ndelay 0.6\nkeystroke "s" using command down')

    def saved():
        if original == hashlib.sha256(path.read_bytes()).hexdigest():
            return None
        values = board["cached_values"](path)
        return values if values.get("A1") == offset else None

    values = wait_for(saved, "recalculate and save the probe")
    keys('keystroke "w" using command down')
    return path, values


def primary(excel, folder, offset):
    path, values = record(excel, folder, {"A1": offset}, board["FORMULAS"])

    def shown(value):
        if value is None:
            return ""
        if type(value) is float and value.is_integer():
            return str(int(value))
        return str(value)

    cells = [
        [shown(values.get(f"{chr(c)}{r}")) for c in range(ord("D"), ord("T") + 1)] for r in range(1, 13)
    ]
    board["check_grid"](cells, board["grid"](offset))
    return {
        "input": offset,
        "cells": cells,
        "workbookSha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def conditions(excel, folder, group):
    formulas = {
        f"{chr(64 + c)}{r}": case["formula"] for r, c, case in board["positions"](group["cases"])
    }
    path, _ = record(excel, folder, group["inputs"], formulas)
    differences = board["differing_cases"](path, group["cases"])
    assert not differences, differences
    return {
        "formulas": len(formulas),
        "matching": True,
        "workbookSha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    parser.add_argument("--excel", type=Path, default=Path("/Applications/Microsoft Excel.app"))
    args = parser.parse_args()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    with (args.excel / "Contents/Info.plist").open("rb") as plist:
        version = plistlib.load(plist)["CFBundleShortVersionString"]
    results = {
        "excelVersion": version,
        "formulas": board["FORMULAS"],
        "snapshots": [primary(args.excel, out, n) for n in (2, 5)],
        "conditions": [conditions(args.excel, out, g) for g in board["conditional_groups"]()],
    }
    (out / "report.json").write_text(json.dumps(results, ensure_ascii=False, indent=2) + "\n")
