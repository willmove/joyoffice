"""Generate Excel controls and review every difference recorded in the fourth batch."""
import argparse
import json
import math
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from check_craft_finance_ets import NS


def controls():
    result = []
    for series in "ABC":
        for season in (0, 1, 4):
            for target in (0, 1, 1.5, 2, 10, 19.5, 20, 23.5, 24, 24.5, 25):
                result.append(f"FORECAST.ETS({target},{series}1:{series}24,D1:D24,{season})")
        for target in (0, 1, 20, 23.5, 24, 25):
            result.append(f"FORECAST.ETS.CONFINT({target},{series}1:{series}24,D1:D24,0.95,0)")
    for season in (-1, 24, 25, 8760, 8761, 9000):
        result.append(f"FORECAST.ETS(25,A1:A24,D1:D24,{season})")

    # A zero coupon isolates the redemption exponent. Zero yield isolates net
    # accrued interest. Combining those observations with the priced coupon
    # isolates the gross first payment independently of the clean price.
    for issue, settlements in (
        ("2007,1,20", ("2007,2,28", "2007,10,10", "2008,4,10", "2008,9,1", "2008,11,11", "2009,2,27")),
        ("2008,9,1", ("2008,11,11",)),
        ("2008,10,15", ("2008,11,11",)),
    ):
        for settlement in settlements:
            dates = f"DATE({settlement}),DATE(2021,3,1),DATE({issue}),DATE(2009,3,1)"
            for basis in range(5):
                for coupon, yld in ((0, 0.0625), (0.0785, 0), (0.0785, 0.0625)):
                    result.append(f"ODDFPRICE({dates},{coupon},{yld},100,2,{basis})")
    return result


def workbook(source, output):
    expressions = controls()
    sheet = "xl/worksheets/sheet1.xml"
    with ZipFile(source) as original, ZipFile(output, "x") as target:
        root = ET.fromstring(original.read(sheet))
        rows = root.find(NS + "sheetData")
        for row in rows:
            for cell in list(row):
                if cell.get("r", "").startswith("G"):
                    row.remove(cell)
        for i, formula in enumerate(expressions, 2):
            cell = ET.SubElement(rows[i - 1], NS + "c", r=f"G{i}")
            ET.SubElement(cell, NS + "f").text = ("_xlfn." if formula.startswith("FORECAST.") else "") + formula
            ET.SubElement(cell, NS + "v").text = "0"
            rows[i - 1][:] = sorted(rows[i - 1], key=lambda c: c.get("r"))
        for item in original.infolist():
            data = ET.tostring(root, encoding="utf-8", xml_declaration=True) if item.filename == sheet else original.read(item)
            target.writestr(item, data)
    print(f"Wrote {len(expressions)} diagnostic formulas")


def same(a, b):
    return a == b or (a[0] == b[0] == "n" and math.isclose(float(a[1]), float(b[1]), rel_tol=1e-9, abs_tol=2e-10))


def review_epoch(previous, current, notes):
    cases = {row["cell"]: row for row in previous["remainingDifferences"]}
    if len(cases) != 76 or set(cases) != set(notes):
        raise ValueError("All 76 old differences require individual review")
    if current["dateSystem"] not in {"1900", "1904"} or current["cases"] != 416:
        raise ValueError("Expected one complete regression workbook")
    review = []
    new_differences = []
    for row in current["results"]:
        cell = row["cell"]
        matches = same(row["excel"], row["candidate"])
        if cell not in cases:
            if not matches:
                new_differences.append(cell)
            continue
        old = cases[cell]
        if (old["formula"] != row["formula"] or not same(old["candidate"], row["prior"])
                or not same(old["excel"], row["excel"]) or not same(old["frozen"], row["frozen"])):
            raise ValueError(f"{cell}: Frozen, previous, or Excel reference has moved")
        review.append(dict(row, status="matched" if matches else "retained", assessment=notes[cell]))
    return dict(dateSystem=current["dateSystem"], matching=sum(same(r["excel"], r["candidate"]) for r in current["results"]),
                checked=len(review), newDifferences=new_differences, crossVersionReadbacks=current["crossVersionReadbacks"], review=review)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    subs = parser.add_subparsers(dest="command", required=True)
    make = subs.add_parser("workbook")
    for name in ("source", "output"):
        make.add_argument(name, type=Path)
    report = subs.add_parser("review")
    for name in ("previous", "current1900", "current1904", "notes", "output"):
        report.add_argument(name, type=Path)
    args = parser.parse_args()
    if args.command == "workbook":
        workbook(args.source, args.output)
    else:
        previous = json.loads(args.previous.read_bytes())["sheet"]["workbooks"]
        notes = json.loads(args.notes.read_bytes())
        result = {
            epoch: review_epoch(previous[epoch], json.loads(path.read_bytes()), notes)
            for epoch, path in (("1900", args.current1900), ("1904", args.current1904))
        }
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
