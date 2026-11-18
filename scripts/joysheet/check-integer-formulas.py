"""Check radix conversions and 48-bit engineering functions against Excel 16.115."""

from pathlib import Path

from formula_workbook import run_family

DATA = Path(__file__).with_name("tests") / "data" / "integer-formulas"


def equal(_formula, actual, expected):
    return float(actual) == float(expected)


if __name__ == "__main__":
    run_family(__doc__, DATA, "12", "进制和位运算.xlsx", equal)
