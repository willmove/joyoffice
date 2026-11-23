"""Compare complex, Bessel, depreciation, and special-math functions with specification results."""

import math
from pathlib import Path

from formula_workbook import run_family

DATA = Path(__file__).parent / "tests" / "data" / "engineering-depreciation"


def equal(formula, actual, expected):
    return math.isclose(float(actual), float(expected), rel_tol=1e-10, abs_tol=1e-11)


if __name__ == "__main__":
    run_family(__doc__, DATA, "12", "工程折旧与特殊函数.xlsx", equal)
