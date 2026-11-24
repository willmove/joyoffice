"""Exercise ETS, securities and time-value functions in a real workbook."""
import argparse
from collections import Counter
import math
from pathlib import Path
import runpy
from xml.etree import ElementTree as ET
from zipfile import ZipFile

from check_craft_pilots import ROOT, cli, digest, sheet_save, write_json

NS = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"


def cases():
    groups = {}
    for f in ("PMT", "PV", "FV"):
        groups[f] = ("0.005,36,8000", "0,12,8000,120,1", "1E-12,12,8000,120,1",
                     "-0.2,3,1000", "-1,3,1000", "-2,3,1000", "0.05,-4,100,0,1",
                     "0.05,0,1000", "0.05,12,1000,,1")
    for f in ("IPMT", "PPMT"):
        groups[f] = ("0.1/12,1,36,8000", "0.1/12,1,36,8000,0,1", "0.1/12,36,36,8000,0,1",
                     "0.1,1.5,12,3000", "0.1,13,12,3000", "0,12,12,3000", "1E-12,2,12,1000")
    for f in ("CUMIPMT", "CUMPRINC"):
        groups[f] = ("0.09/12,360,125000,13,24,0", "0.09/12,360,125000,1,1,1", "0.09/12,360,125000,1,360,1",
                     "0.1,10.9,100,2.1,4.8,1", "0,36,8000,1,1,0", "0.1,10,100,1,11,0", "0.1,10,100,1,1,2")
    groups.update({
        "NPER": ("0.01,-100,-1000,10000,1", "0.01,-100,-1000", "0,-100,1200", "1E-16,-100,1200",
                 "0,0,1200", "-1,-100,1200", "-0.1,-100,1200", "0.01,-12,1200"),
        "RATE": ("48,-200,8000", "12,-100,1200", "12,-100,1250,0,0,-0.05",
                 "5,0,-100,200", "5,0,-200,100,0,-0.1", "12,5,100", "0,-100,1200",
                 "12,-110,1000,0,1,0.9"),
        "FVSCHEDULE": ("1,{0.09,0.11,0.1}", "100,{0,0.25,-0.3}", '100,{0.1,"text"}', "10,E1:E6"),
        "EFFECT": ("0.0525,4", "0.0525,4.9", "0.0525,0", "0,4", "1E-12,365"),
        "NOMINAL": ("0.053543,4", "0.053543,4.9", "0.053543,0", "0,4", "1E-12,365"),
        "ISPMT": ("0.1/12,0,36,8000", "0.1/12,36,36,8000", "0.1,2,0,100"),
        "PDURATION": ("0.025,2000,2200", "0.025,2200,2000", "1E-12,100,101", "0,10,100"),
        "RRI": ("96,10000,11000", "0,10000,11000", "10,1000,999.99999", "1,-10,10", "2,100,0"),
        "DOLLARDE": ("1.02,16", "-1.02,16", "1.2,100", "1.02,0", "1.02,1", "1.02,-1"),
        "DOLLARFR": ("1.125,16", "-1.125,16", "1.02,100", "1.02,0", "1.02,1", "1.02,-1"),
        "NPV": ("0.1,H1:H6", "-1,H1:H6"), "XNPV": ("0.09,H1:H6,I1:I6", "0,H1:H6,I1:I6"),
        "IRR": ("H1:H6", "H1:H6,-0.3", "E1:E6"), "XIRR": ("H1:H6,I1:I6", "H1:H6,I1:I6,-0.3"),
        "MIRR": ("H1:H6,0.1,0.12", "E1:E6,0.1,0.1"),
    })
    c = "DATE(2008,2,15),DATE(2016,11,15)"
    month = "DATE(2012,2,28),DATE(2017,8,31)"
    short = "DATE(2008,2,15),DATE(2008,6,15)"
    disc = "DATE(2008,2,16),DATE(2008,3,1)"
    for f in ("COUPDAYBS", "COUPDAYS", "COUPDAYSNC", "COUPNUM", "COUPNCD", "COUPPCD"):
        groups[f] = tuple(f"{month},{freq},{basis}" for freq,basis in ((2,0),(2,1),(4,2),(4,3),(2,4),(3,1),(1,5)))
    for f, pair in {"DISC":"97,100", "PRICEDISC":"0.0525,100", "YIELDDISC":"99.795,100",
                    "INTRATE":"1000,1075", "RECEIVED":"1000,0.0575"}.items():
        groups[f] = tuple(f"{disc},{pair},{b}" for b in range(5))
    for f, parameter in (("TBILLPRICE","0.09"), ("TBILLYIELD","98.45"), ("TBILLEQ","0.0914")):
        groups[f] = tuple(f"{start},{stop},{parameter}" for start, stop in (
            ("DATE(2008,3,31)", "DATE(2008,6,1)"), ("DATE(2008,3,31)", "DATE(2009,3,30)"),
            ("DATE(2008,3,31)", "DATE(2009,6,1)"), ("DATE(2008,6,1)", "DATE(2008,3,31)")))
    groups.update({
        "PRICEMAT": tuple(f"{disc},DATE(2007,11,11),0.061,0.061,{b}" for b in range(5)),
        "YIELDMAT": tuple(f"{disc},DATE(2007,11,11),0.061,100.0123,{b}" for b in range(5)),
        "PRICE": tuple(f"{s},0.0575,0.065,100,2,{b}" for s in (c,short) for b in range(5)),
        "YIELD": tuple(f"{s},0.0575,95.04287,100,2,{b}" for s in (c,short) for b in range(5)) +
                 (f"{c},0.01,110,100,2,0", f"{short},0,120,100,2,1"),
        "DURATION": tuple(f"{c},0.08,0.09,2,{b}" for b in range(5)),
        "MDURATION": tuple(f"{c},0.08,0.09,2,{b}" for b in range(5)),
        "ACCRINT": tuple(f"DATE(2008,3,1),DATE(2008,8,31),DATE(2008,5,1),0.1,1000,2,{b},{m}"
                         for b in range(5) for m in ("TRUE","FALSE")),
        "ACCRINTM": tuple(f"{disc},0.1,1000,{b}" for b in range(5)),
    })
    for f, q in (("ODDFPRICE","0.0625"),("ODDFYIELD","84.50")):
        groups[f] = tuple(f"DATE(2008,11,11),DATE(2021,3,1),DATE({issue}),DATE(2009,3,1),0.0785,{q},100,2,{b}"
                          for issue in ("2008,10,15","2007,1,20") for b in range(5))
    for f, q in (("ODDLPRICE","0.0405"),("ODDLYIELD","99.875")):
        groups[f] = tuple(f"DATE(2008,2,7),DATE(2008,6,15),DATE(2007,10,15),0.0375,{q},100,2,{b}" for b in range(5))
    for series in ("A1:A24", "B1:B24", "C1:C24"):
        timeline = f"{series},D1:D24"
        groups.setdefault("FORECAST.ETS", []).extend(f"{t},{timeline},{m}" for t in (20.,24.5,25.,28.,33.) for m in (0,1,4))
        groups.setdefault("FORECAST.ETS.CONFINT", []).extend(f"{t},{timeline},{q},{m}" for t in (25,33) for q,m in ((0.8,0),(0.95,1),(0.99,4),(1,0)))
        groups.setdefault("FORECAST.ETS.SEASONALITY", []).append(timeline)
        groups.setdefault("FORECAST.ETS.STAT", []).extend(f"{timeline},{k},{m}" for k in range(1,10) for m in (0,4))
    groups["FORECAST.ETS"].extend(["25,A1:A20,D1:D24", "25,A1:A24,D1:D24,8761",
                                   "13,E1:E8,F1:F8,0,0", "13,E1:E8,F1:F8,0,1"])
    return [f"{name}({args})" for name, inputs in groups.items() for args in inputs]


def workbook(path, date1904=False):
    template = ROOT / "scripts/joysheet/tests/data/engineering-depreciation/base.xlsx"
    ws = ET.Element(NS + "worksheet")
    data = ET.SubElement(ws, NS + "sheetData")
    special = ("PDURATION", "RRI")
    extra = ([10, 20, 40, 50, 60, 80, 90, 100], [1, 2, 4, 5, 6, 8, 9, 10],
             [-10000, 3100, 2700, 2200, 4600, 1200], [44000, 44103, 44751, 45107, 46001, 46300])
    input_columns = [([2*k + 3 for k in range(1,25)], "A"),
                     ([30+k+[8,-3,4,-9][(k-1)%4] for k in range(1,25)], "B"),
                     ([55+2*k+[1,0,4,-3,2][-k%5]+(k%3) for k in range(1,25)], "C"),
                     (range(1,25), "D"), *zip(extra, "EFHI")]
    expressions = cases()
    for i in range(1, len(expressions) + 2):
        row = ET.SubElement(data, NS+"row", r=str(i))
        for values, column in input_columns:
            if i <= len(values):
                value = ET.SubElement(row, NS+"c", r=f"{column}{i}")
                ET.SubElement(value, NS+"v").text = str(values[i-1])
        if i >= 2:
            value = ET.SubElement(row, NS+"c", r=f"G{i}")
            expr = expressions[i-2]
            prefix = "_xlfn." if expr.startswith(("FORECAST.ETS", *special)) else ""
            ET.SubElement(value, NS+"f").text = prefix + expr
            ET.SubElement(value, NS+"v").text = "0"
        row[:] = sorted(row, key=lambda c: c.get("r"))
    with ZipFile(template) as original, ZipFile(path, "x") as target:
        for name in original.namelist():
            payload = original.read(name)
            if name == "xl/worksheets/sheet1.xml":
                payload=ET.tostring(ws, encoding="utf-8", xml_declaration=True)
            if name == "xl/workbook.xml" and date1904:
                item=ET.fromstring(payload)
                properties=item.find(NS+"workbookPr")
                if properties is None:
                    properties=ET.Element(NS+"workbookPr")
                    item.insert(0,properties)
                properties.set("date1904","1")
                payload=ET.tostring(item,encoding="utf-8",xml_declaration=True)
            target.writestr(original.getinfo(name),payload)


def compare(source, excel, frozen, prior, candidate, out):
    out.mkdir(parents=True)
    work = runpy.run_path(str(ROOT / "scripts/joysheet/formula_workbook.py"))

    def epoch(path):
        with ZipFile(path) as package:
            properties = ET.fromstring(package.read("xl/workbook.xml")).find(NS + "workbookPr")
            return properties is not None and properties.get("date1904") in {"1", "true"}

    assert epoch(source) == epoch(excel)
    data = {"excel": work["cells"](excel)}
    builds = {"frozen": frozen, "prior": prior}
    if candidate:
        builds["candidate"] = candidate
    for key, path in builds.items():
        data[key] = sheet_save(path,source,out/f"{key}.xlsx")
    expressions = cases()
    assert all(len(values) == len(expressions) for values in data.values())

    def same(a, b):
        return a == b or (a[0] == b[0] == "n" and math.isclose(float(a[1]), float(b[1]), rel_tol=1e-9, abs_tol=2e-10))

    rows = []
    for i, formula in enumerate(expressions):
        name = formula.split("(", 1)[0]
        values = {label: list(records[i][1:]) for label, records in data.items()}
        assert all(records[i][0].split("(", 1)[0] == name for records in data.values()), formula
        rows.append(dict(cell=f"G{i+2}", formula=formula, **values))
    misses = {label: [row for row in rows if not same(row[label], row["excel"])] for label in builds}
    by_function = {label: dict(Counter(row["formula"].split("(", 1)[0] for row in items)) for label, items in misses.items()}
    report = dict(sourceSha256=digest(source), excelSha256=digest(excel), cases=len(rows),
                  dateSystem="1904" if epoch(source) else "1900",
                  binarySha256={label: digest(cli(build, "sheet")) for label, build in builds.items()},
                  registeredNames=len({s.split("(", 1)[0] for s in expressions}),
                  differencesByFunction=by_function, results=rows)
    if candidate:
        report["changedCells"] = [row["cell"] for row in rows if not same(row["candidate"], row["prior"])]
        previous = {row["cell"] for row in misses["prior"]}
        current = {row["cell"] for row in misses["candidate"]}
        report["resolvedExcelDifferences"] = [row["cell"] for row in rows if row["cell"] in previous - current]
        report["newExcelDifferences"] = [row["cell"] for row in rows if row["cell"] in current - previous]
        for name, reader in (("frozen", frozen), ("candidate", candidate)):
            for writer in ("frozen", "candidate"):
                records = sheet_save(reader, out / f"{writer}.xlsx", out / f"{writer}-to-{name}.xlsx")
                assert len(records) == len(rows)
                assert all(same(list(actual[1:]), expected[name]) for actual, expected in zip(records, rows)), (writer, name)
        report["crossVersionReadbacks"] = 4
    write_json(out / "report.json", report)


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output",type=Path)
    parser.add_argument("--date1904",action="store_true")
    for arg in ("source","excel","frozen","prior","candidate"):
        parser.add_argument("--"+arg,type=Path)
    args=parser.parse_args()
    if args.source:
        if not all((args.excel,args.frozen,args.prior)):
            parser.error("comparison needs --excel, --frozen and --prior")
        compare(args.source,args.excel,args.frozen,args.prior,args.candidate,args.output)
    else:
        workbook(args.output,args.date1904)
        print(f"Wrote {len(cases())} formulas")
