"""Compare JoySheet cash-flow recalculation and native edits with Excel's saved numeric and error caches."""

import math
from pathlib import Path

from formula_workbook import run_family

DATA = Path(__file__).with_name("tests") / "data" / "cashflows"


def equal(formula, left, right):
    if left == right:
        return True
    if formula.startswith(("IRR(", "XIRR(")):
        # Excel's iterative rates can end on either side of an exact zero.
        return math.isclose(float(left), float(right), rel_tol=1e-8, abs_tol=1e-8)
    return math.isclose(float(left), float(right), rel_tol=2e-12, abs_tol=1e-21)


if __name__ == "__main__":
    run_family(__doc__, DATA, "-120", "现金流 重算.xlsx", equal)
