"""Compare the matrix (MMULT/MDETERM/MINVERSE/MUNIT) and classic text families with specification results."""

import math
from pathlib import Path

from formula_workbook import run_family

DATA = Path(__file__).parent / "tests" / "data" / "matrix-classic-text"


def equal(formula, actual, expected):
    return math.isclose(float(actual), float(expected), rel_tol=1e-11, abs_tol=1e-12)


if __name__ == "__main__":
    run_family(__doc__, DATA, "5", "矩阵与经典文本.xlsx", equal)
