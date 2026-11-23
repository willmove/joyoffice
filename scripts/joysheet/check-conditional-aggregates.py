"""Compare the conditional aggregate and criteria families with independently saved Excel results."""

import math
from pathlib import Path

from formula_workbook import run_family

DATA = Path(__file__).parent / "tests" / "data" / "conditional-aggregates"


def equal(formula, actual, expected):
    return math.isclose(float(actual), float(expected), rel_tol=2e-12, abs_tol=0.0)


if __name__ == "__main__":
    run_family(__doc__, DATA, "12", "条件聚合.xlsx", equal)
