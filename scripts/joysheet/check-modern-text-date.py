"""Compare modern text/regex/formatting and calendar date families with specification results."""

import math
from pathlib import Path

from formula_workbook import run_family

DATA = Path(__file__).parent / "tests" / "data" / "modern-text-date"


def equal(formula, actual, expected):
    return math.isclose(float(actual), float(expected), rel_tol=1e-11, abs_tol=1e-12)


if __name__ == "__main__":
    run_family(__doc__, DATA, "2024", "现代文本与日期.xlsx", equal)
