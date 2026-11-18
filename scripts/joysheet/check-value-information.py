"""Check value classification, reference types and parity against Excel 16.115."""

from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from formula_workbook import run_family

DATA = Path(__file__).with_name("tests") / "data" / "value-information"


def equal(_formula, actual, expected):
    return float(actual) == float(expected)


def file_contract(source, writers):
    def inspect(path):
        with ZipFile(path) as package:
            sheet = ET.fromstring(package.read("xl/worksheets/sheet1.xml"))
            bags = ET.fromstring(package.read("xl/featurePropertyBag/featurePropertyBag.xml"))
        formulas = {}
        for cell in sheet.iterfind(".//{*}c"):
            expr = cell.find("{*}f")
            if expr is not None and (expr.text or "").startswith("TYPE("):
                formulas[cell.get("r")] = (cell.get("cm"), expr.get("t"), expr.get("ref"))
        mapper = bags.find("{*}bag[@type='XFComplements']")
        assert mapper is not None and formulas
        return formulas, mapper.get("extRef")

    original, name = inspect(source)
    previous, old_name = inspect(writers["baseline"])
    current, current_name = inspect(writers["candidate"])
    assert original == current and previous.keys() == original.keys()
    changed = sum(mode != previous[cell] for cell, mode in original.items())
    assert changed and old_name is None and name == current_name == "XFComplementsMapperExtRef"
    return {"typeFormulas": len(original), "previousWriterChangedEntryModes": changed,
            "candidateWriterChangedEntryModes": 0, "checkboxMapper": current_name}


if __name__ == "__main__":
    run_family(__doc__, DATA, "12", "类型与引用.xlsx", equal, previous_writer_data=DATA / "previous-writer", inspect_writers=file_contract)
