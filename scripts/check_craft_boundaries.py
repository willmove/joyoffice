"""Generate Excel controls and pin the result of reviewing the 22 probability exceptions."""
import argparse
import json
import math
from pathlib import Path
import runpy
from statistics import NormalDist
from xml.etree import ElementTree as ET
from zipfile import ZIP_DEFLATED, ZipFile

from check_craft_stage1 import ROOT, NS
from check_craft_pilots import digest, sheet_save, write_json


def cases():
    previous = ROOT / "docs/joyoffice/validation/20261124-craft-stage1/verification.json"
    exceptions = json.loads(previous.read_bytes())["sheet"]["candidateVsExcel"]
    rows = [r["formula"] for r in exceptions]

    def add(names, args):
        rows.extend(f"{n}({a})" for n in names.split() for a in args)

    add("CHISQ.INV.RT CHIINV", [f"1E-{p},5" for p in (15, 16, 17, 20, 50, 100, 300)]
        + ["1E-40,1", "1E-40,2", "1E-40,10"])
    add("BINOM.INV CRITBINOM", ["1,0.5,0.499999999999999", "1,0.5,0.500000000000001",
        "3,0.5,0.5", "1,0.25,0.75", "2,0.5,0.25", "2,0.5,0.75"])
    add("GAMMA.DIST GAMMADIST", ["0,0.5,2,FALSE", "0,2,2,FALSE", "0,1,2,TRUE",
        "1E-20,1,2,FALSE", "0,1,1,FALSE"])
    add("BETA.DIST", ["0,1,1,FALSE", "1,1,1,FALSE", "0,0.5,2,FALSE", "1,2,0.5,FALSE",
        "0,2,2,FALSE", "1,2,2,FALSE", "0,1,2,TRUE", "1,2,1,TRUE",
        "-2,1,2,FALSE,-2,3", "3,2,1,FALSE,-2,3"])
    add("BETA.INV BETAINV", ["1,2,2", "1,1,1", "1,2,0.5,-2,3", "0.999999999999999,1,2"])
    add("WEIBULL.DIST WEIBULL", ["0,2,5,FALSE", "0,1,5,TRUE", "1E-20,1,5,FALSE", "1E-20,0.5,5,FALSE"])
    add("HYPGEOM.DIST", ["3,9,10,10,FALSE", "9,9,10,10,TRUE", "0,4,0,20,TRUE",
        "2,4,0,20,FALSE", "0,0,3,20,FALSE", "0,0,0,20,TRUE", "0,4,0,20,FALSE",
        "5,4,3,20,TRUE", "0,4,3,3,TRUE", "0,0,0,0,TRUE", "0,2,4,3,TRUE"])
    add("HYPGEOMDIST", ["0,4,0,20", "0,0,3,20", "0,0,0,20", "3,9,10,10", "9,9,10,10"])
    add("NEGBINOM.DIST", ["0,1,0,TRUE", "0,3,1E-14,FALSE", "0,3,0.99999999999999,TRUE",
        "3,1,1,FALSE", "3,1,1,TRUE", "5,3,0,FALSE", "0,1,1,FALSE"])
    add("NEGBINOMDIST", ["0,1,0", "0,3,1", "3,1,1", "5,3,0"])
    return list(dict.fromkeys(rows))


def workbook(path, expressions=None):
    template = ROOT / "scripts/joysheet/tests/data/engineering-depreciation/base.xlsx"
    ns = f"{{{NS}}}"
    ws = ET.Element(ns + "worksheet")
    data = ET.SubElement(ws, ns + "sheetData")
    for i, formula in enumerate(cases() if expressions is None else expressions, 2):
        row = ET.SubElement(data, ns + "row", r=str(i))
        cell = ET.SubElement(row, ns + "c", r=f"G{i}")
        prefix = "_xlfn." if "." in formula.split("(", 1)[0] else ""
        ET.SubElement(cell, ns + "f").text = prefix + formula
        ET.SubElement(cell, ns + "v").text = "0"
    with ZipFile(template) as source, ZipFile(path, "x", ZIP_DEFLATED) as target:
        for name in source.namelist():
            data = ET.tostring(ws, encoding="utf-8", xml_declaration=True) if name == "xl/worksheets/sheet1.xml" else source.read(name)
            target.writestr(source.getinfo(name), data)


REASONS = {
    "CHISQ.INV.RT": "有限反解经半整数或整数上不完全 Gamma 递推独立校验",
    "CHIINV": "与 CHISQ.INV.RT 为相同右尾反解，独立回代通过",
    "BINOM.INV": "按公开定义取第一个累积概率大于或等于阈值的整数",
    "CRITBINOM": "旧别名沿用公开定义的含等号阈值",
    "GAMMA.DIST": "形状参数为 1 时零点右极限和指数密度均为 1/scale",
    "GAMMADIST": "旧别名保留指数密度的零点值",
    "BETA.DIST": "端点形状参数为 1 时，有限的单侧密度为另一形状参数除以区间宽度",
    "BETA.INV": "公开参数范围允许概率 1，分位数为区间上界",
    "BETAINV": "旧别名保留概率 1 对应区间上界",
    "WEIBULL.DIST": "形状为 1 时密度是 1/scale，小于 1 时发散并报告数字错误",
    "WEIBULL": "旧别名保留相同的零点密度及非有限值契约",
    "HYPGEOM.DIST": "公开契约拒绝样本数或总体成功数为零，以及超出支持集的样本成功数",
    "HYPGEOMDIST": "旧别名继续拒绝总体成功数为零及超出支持集的输入",
    "NEGBINOM.DIST": "公开参数范围包含 0 和 1，使用成功概率端点的退化分布",
}


def compare(source, excel, frozen, prior, candidate, out):
    out.mkdir(parents=True)
    read = runpy.run_path(str(ROOT / "scripts/joysheet/formula_workbook.py"))["cells"]
    formulas = cases()
    original = digest(source)
    saved = {
        "frozen": sheet_save(frozen, source, out / "frozen.xlsx"),
        "prior": sheet_save(prior, source, out / "prior.xlsx"),
        "candidate": sheet_save(candidate, source, out / "candidate.xlsx"),
        "excel": read(excel),
    }
    assert all(len(rows) == len(formulas) for rows in saved.values())
    assert digest(source) == original
    assert all(row[0].split("(", 1)[0] == formula.split("(", 1)[0]
               for rows in saved.values() for row, formula in zip(rows, formulas))

    def same(left, right):
        return left == right or (
            left[0] == right[0] == "n" and math.isclose(float(left[1]), float(right[1]), rel_tol=1e-10)
        )

    previous = ROOT / "docs/joyoffice/validation/20261124-craft-stage1/verification.json"
    archived = json.loads(previous.read_bytes())["sheet"]["candidateVsExcel"]
    review = []
    controls = []
    chi_verified = 0
    for idx, formula in enumerate(formulas):
        values = {key: list(rows[idx][1:]) for key, rows in saved.items()}
        name = formula.split("(", 1)[0]
        row = dict(cell=f"G{idx + 2}", formula=formula, **values)
        if idx < len(archived):
            assert formula == archived[idx]["formula"]
            assert same(values["excel"], archived[idx]["expected"]), row
            assert same(values["candidate"], archived[idx]["actual"]), row
            row.update(originalCell=archived[idx]["cell"], decision="保留冻结版结果", reasoning=REASONS[name])
            review.append(row)
        else:
            if name in {"BINOM.INV", "CRITBINOM"} and formula.endswith("(3,0.5,0.5)"):
                assert values["prior"] == ["n", "2"], row
                assert values["candidate"] == values["excel"] == values["frozen"] == ["n", "1"], row
            else:
                assert same(values["candidate"], values["prior"]), row
            controls.append(row)
        if name in {"CHISQ.INV.RT", "CHIINV"}:
            p, degrees = (float(x) for x in formula.split("(")[1][:-1].split(","))
            assert values["candidate"][0] == "n"
            x = 0.5 * float(values["candidate"][1])
            shape = 0.5 if degrees % 2 else 1.0
            right = math.erfc(math.sqrt(x)) if degrees % 2 else math.exp(-x)
            while shape < 0.5 * degrees:
                right += math.exp(shape * math.log(x) - x - math.lgamma(shape + 1.0))
                shape += 1.0
            assert math.isclose(p, right, rel_tol=3e-12), (formula, right)
            row["independentChiSquareRightTail"] = right
            chi_verified += 1
    write_json(out / "report.json", {
        "sourceSha256": original, "excelSha256": digest(excel), "reviewed": review,
        "adjacentControls": controls, "independentChiSquareChecks": chi_verified,
        "correctedSymmetricBinomialMedians": 2,
    })


def check_kernels(candidate, out):
    """Check against the standard library and finite probability sums."""
    probes = []
    for x in (1e-300, 0.003, 0.25, 0.5, 0.99, 1., 1.00000000000001, 1.125, 1.5, 2.,
              2.5, 7.9, 8., 25.25, 100., 170.5, 1200., 1e6):
        probes.append((f"GAMMALN.PRECISE({x})", math.lgamma(x), 5e-15))
    for x in (-22., -8., -3., -1.2, -0.5, -0.49, -1e-50, 0., 0.1, 0.49, 0.5, 0.51, 1., 2., 3., 7., 15., 25.):
        probes.extend([(f"ERF({x})", math.erf(x), 0.), (f"ERFC({x})", math.erfc(x), 0.),
                       (f"NORM.S.DIST({x},TRUE)", math.erfc(-x / math.sqrt(2)) * 0.5, 0.)])
    for p in (1e-300, 1e-200, 1e-100, 1e-50, 1e-20, 1e-6, 0.005, 0.1, 0.24999,
              0.25, 0.5, 0.70, 0.75, 0.85, 0.995, 0.999999999999999):
        probes.append((f"NORM.S.INV({p})", NormalDist().inv_cdf(p), 0.))
    # Compute the adjacent binary64 values in the workbook to keep the 15-digit
    # worksheet literal rule from rounding either one to 0.5 during import.
    for expr, p in (("0.5-2^(-54)", math.nextafter(0.5, 0.)),
                    ("0.5+2^(-53)", math.nextafter(0.5, 1.))):
        probes.append((f"NORM.S.INV({expr})", NormalDist().inv_cdf(p), 0.))
    for a, b in ((1, 100), (2, 3), (2, 19), (5, 12), (13, 20), (40, 40)):
        n = a + b - 1
        for p in (0.001, 0.1, 0.2, 0.5, 0.8, 0.99):
            value = math.fsum(math.comb(n, k) * p**k * (1-p)**(n-k) for k in range(a, n+1))
            probes.append((f"BETA.DIST({p},{a},{b},TRUE)", value, 0.))
    for a in (1, 2, 3, 9, 25, 100):
        for x in (0.01, 0.3, 1., 8., 30., 40., 110., 600.):
            # Integer-shape gamma CDF equals a Poisson survival sum.
            value = math.fsum(math.exp(-x + k * math.log(x) - math.lgamma(k + 1))
                              for k in range(a, max(a + 500, int(x) + 600)))
            probes.append((f"GAMMA.DIST({x},{a},1,TRUE)", value, 0.))
    out.mkdir(parents=True)
    source = out / "kernel-probes.xlsx"
    workbook(source, (f for f, _, _ in probes))
    actual = sheet_save(candidate, source, out / "kernel-results.xlsx")
    assert len(probes) == len(actual)
    counts = {}
    for (formula, expected, atol), (written, kind, value) in zip(probes, actual):
        name = formula.split("(", 1)[0]
        assert written.split("(", 1)[0] == name
        assert kind == "n" and math.isclose(float(value), expected, rel_tol=1e-10, abs_tol=atol), (
            formula, value, expected)
        counts[name] = counts.get(name, 0) + 1
    write_json(out / "report.json", {"sourceSha256": digest(source), "byFunction": counts,
                                     "comparisons": len(actual)})


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--kernels", action="store_true", help="compare the candidate with independent mathematical references")
    for option in ("source", "excel", "frozen", "prior", "candidate"):
        parser.add_argument("--" + option, type=Path)
    args = parser.parse_args()
    if args.kernels:
        if not args.candidate:
            parser.error("--kernels requires --candidate")
        check_kernels(args.candidate.resolve(), args.output.resolve())
    elif args.source:
        if not all((args.excel, args.frozen, args.prior, args.candidate)):
            parser.error("comparison requires --source, --excel, --frozen, --prior and --candidate")
        compare(args.source.resolve(), args.excel.resolve(), args.frozen.resolve(), args.prior.resolve(),
                args.candidate.resolve(), args.output.resolve())
    else:
        workbook(args.output)
        print(f"Wrote {len(cases())} formulas")
