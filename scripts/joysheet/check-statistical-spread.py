"""Compare the variance, deviation, skewness and kurtosis families with independently saved Excel results."""

import math
from pathlib import Path

from formula_workbook import run_family

DATA = Path(__file__).parent / "tests" / "data" / "statistical-spread"


def equal(formula, actual, expected):
    # Skew of a symmetric population is zero up to floating-point roundoff.
    absolute = 2e-13 if formula.startswith(("SKEW", "KURT")) else 0.0
    return math.isclose(float(actual), float(expected), rel_tol=2e-12, abs_tol=absolute)


if __name__ == "__main__":
    run_family(__doc__, DATA, "12", "统计分布.xlsx", equal)
