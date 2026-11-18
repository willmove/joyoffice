"""Check Excel's multivariate arrays, typed errors and numerical conditioning in two generations of JoySheet."""

from decimal import Decimal, localcontext
import math
from pathlib import Path
import re
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from formula_workbook import cells, differences, run_family

DATA = Path(__file__).parent / "tests" / "data" / "multivariate-regression"
SPILLS = {"AH2": "AH2:AJ6", "AL2": "AL2:AN6", "AP2": "AP2:AP5", "AR2": "AR2:AU2"}


def strict(_, a, b):
    return math.isclose(float(a), float(b), rel_tol=2e-10, abs_tol=1e-12)


def agrees(formula, a, b):
    if strict(formula, a, b):
        return True
    actual, reference = float(a), float(b)
    if ",M2:N9,TRUE,TRUE)" in formula:
        # Condition number ~10^5. Excel's intercept SE also loses precision
        # when subtracting nearly equal terms in its covariance matrix.
        tol = 2e-6 if formula.endswith(",2,3)") else 4e-9
        return math.isclose(actual, reference, rel_tol=tol, abs_tol=1e-12)
    if formula.startswith("INDEX(TREND(W2:W9,Q2:R9,Q4:R7,TRUE)"):
        # A rounding unit at the scale of the uncentered prediction (10^11).
        return abs(actual - reference) <= math.ulp(1e11)
    if re.fullmatch(r"INDEX\(LINEST\(V2:V9,(?:J2:J9|AB2:AC9),TRUE,TRUE\),4,1\)", formula):
        # y = 2x + 1 exactly. Finite residual roundoff determines the F
        # statistic. Preserve the independent result type and reciprocal scale.
        return math.isfinite(actual) and min(actual, reference) > 1 / math.ulp(1.)**2
    return False


def read_sheet(path):
    with ZipFile(path) as archive:
        root = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
    return {cell.get("r"): cell for cell in root.iterfind(".//{*}c")}


def solve_two(source, xcols, ycol):
    # Independent two-variable solution evaluated at 75 decimal digits. The
    # numeric tokens round to binary64 before calculation, as in the workbook.
    def column(name):
        return [Decimal.from_float(float(source[f"{name}{r}"].findtext("{*}v"))) for r in range(2, 10)]

    x, z, y = map(column, [*xcols, ycol])
    if ycol == "X":
        y = [v.ln() for v in y]
    n = Decimal(len(y))
    mx, mz, my = (sum(col) / n for col in (x, z, y))
    xc, zc, yc = [[v - mean for v in col] for col, mean in ((x, mx), (z, mz), (y, my))]

    def dot(a, b):
        return sum(v * w for v, w in zip(a, b, strict=True))

    xx, zz, xz = dot(xc, xc), dot(zc, zc), dot(xc, zc)
    xy, zy = dot(xc, yc), dot(zc, yc)
    det = xx * zz - xz * xz
    beta, gamma = (zz * xy - xz * zy) / det, (xx * zy - xz * xy) / det
    predictions = [my + beta * a + gamma * b for a, b in zip(xc, zc, strict=True)]
    residual = sum((v - w)**2 for v, w in zip(y, predictions, strict=True))
    total = dot(yc, yc)
    df = n - 3
    variance = residual / df
    model = total - residual
    intercept_se = (variance * (1 / n + (zz * mx**2 - 2 * xz * mx * mz + xx * mz**2) / det)).sqrt()
    return [[(xx * variance / det).sqrt(), (zz * variance / det).sqrt(), intercept_se],
            [model / total, variance.sqrt()], [model / 2 / variance, df], [model, residual]], predictions


def check_spills(reference, saved):
    column_index = lambda letters: sum((ord(c) - 64) * 26**i for i, c in enumerate(reversed(letters)))
    def xy(address):
        m = re.fullmatch(r"([A-Z]+)([0-9]+)", address)
        return column_index(m[1]), int(m[2])

    checked = 0
    for anchor, rect in SPILLS.items():
        for states in [reference, saved]:
            entry = states[anchor].find("{*}f")
            assert entry is not None and entry.get("t") == "array" and entry.get("ref", anchor) == rect, (anchor, rect)
        x1, y1 = xy(anchor)
        x2, y2 = xy(rect.split(":")[1])
        for address, cell in reference.items():
            x, y = xy(address)
            if not (x1 <= x <= x2 and y1 <= y <= y2):
                continue
            other = saved[address]
            kind, value = cell.get("t", "n"), cell.findtext("{*}v")
            assert kind == other.get("t", "n")
            actual = other.findtext("{*}v")
            assert value == actual or (kind == "n" and strict("", actual, value)), (address, actual, value)
            checked += 1
    assert checked == 38
    return checked


def inspect(source_file, writers):
    reference, saved = read_sheet(source_file), read_sheet(writers["candidate"])
    exact_errors = {}
    with localcontext() as context:
        context.prec = 75
        for label, offset in [("W", 82), ("X", 301)]:
            stats, _ = solve_two(reference, "MN", label)
            largest = 0.
            for r, row in enumerate(stats, 1):
                for col, value in enumerate(row):
                    a = float(saved[f"G{offset + r * 3 + col}"].findtext("{*}v"))
                    largest = max(largest, abs(a / float(value) - 1))
                    assert math.isclose(a, float(value), rel_tol=4e-9, abs_tol=1e-13)
            exact_errors[label + "WorstRelativeStatisticDifference"] = largest
        _, predicted = solve_two(reference, "QR", "W")
        delta = max(abs(float(saved[f"G{486+i}"].findtext("{*}v")) - float(predicted[2+i])) for i in range(4))
        assert delta < 2e-12
        exact_errors["largeOffsetPredictionWorstAbsoluteDifference"] = delta
    out = cells(source_file)
    return {"decimalPrecision": 75, **exact_errors,
            "strictExcelDifferences": {key: differences(cells(value), out, strict) for key, value in writers.items()},
            "arrayExtents": SPILLS, "arrayCellsChecked": check_spills(reference, saved)}


if __name__ == "__main__":
    run_family(__doc__, DATA, "12", "多元回归.xlsx", agrees, inspect_writers=inspect)
