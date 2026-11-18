"""Compare JoySheet cash-flow recalculation and native edits with Excel's saved numeric and error caches."""

import argparse
from collections import Counter
import json
import math
from pathlib import Path
import runpy

CALENDAR = runpy.run_path(str(Path(__file__).with_name("check-workday-calendar.py")))
DATA = Path(__file__).with_name("tests") / "data" / "cashflows"


def equal(formula, left, right):
    if left == right:
        return True
    if formula.startswith(("IRR(", "XIRR(")):
        # Excel's iterative rates can end on either side of an exact zero.
        return math.isclose(float(left), float(right), rel_tol=1e-8, abs_tol=1e-8)
    return math.isclose(float(left), float(right), rel_tol=2e-12, abs_tol=1e-21)


def differences(rows, reference):
    assert len(rows) == len(reference) and rows
    failures = []
    for i, ((f, t, v), (expr, kind, expected)) in enumerate(zip(rows, reference), 2):
        assert f == expr, (i, f, expr)
        if t != kind or (v != expected and (kind == "e" or not equal(expr, v, expected))):
            failures.append((f"G{i}", expr, expected, v))
    return failures


def exchange(old, new, source, folder):
    digest, read, save = (CALENDAR[k] for k in ("digest", "cells", "save"))
    original = digest(source)
    expected = read(source)
    by_writer = {}
    for label, writer in (("baseline", old), ("candidate", new)):
        output = folder / f"{source.stem}-{label}.xlsx"
        save(writer, source, output, "en")
        by_writer[label] = read(output)
    errors = {key: differences(rows, expected) for key, rows in by_writer.items()}
    assert not errors["candidate"], errors["candidate"]
    # Force each reader to recalculate and save the other writer's formulas.
    for label, reader in (("baseline", old), ("candidate", new)):
        partner = "baseline" if label == "candidate" else "candidate"
        output = folder / f"{source.stem}-{partner}-to-{label}.xlsx"
        save(reader, folder / f"{source.stem}-{partner}.xlsx", output, "en")
        assert not differences(read(output), by_writer[label])
    assert digest(source) == original
    return {
        "sourceSha256": original,
        "independentByFunction": dict(Counter(f.split("(")[0] for f, _, _ in expected)),
        "baselineDifferences": errors["baseline"],
        "candidateDifferences": len(errors["candidate"]),
        "bothGenerationsRecalculatedBothWriters": True,
    }


def native(program, folder):
    original, edited = [DATA / (tag + ".xlsx") for tag in ("base", "edit")]
    source_hash = CALENDAR["digest"](original)
    states = [CALENDAR["cells"](book) for book in (original, edited)]
    changed = {i for i, (a, b) in enumerate(zip(*states)) if a != b}
    assert changed
    indices = sorted(changed | set(range(0, len(states[0]), 29)))

    def launch(tag):
        number = CALENDAR["port"]()
        profile = folder / (tag + "-profile")
        return CALENDAR["SheetControl"](number, profile), CALENDAR["app"](
            program, ["--control", str(number), "--diagnostics"], folder,
            {"JOYSHEET_CONFIG_DIR": str(profile), "LC_ALL": CALENDAR["REGIONS"]["en"]},
            tag + ".log", unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        )

    def inspect(call, state):
        call("ui.awaitIdle")
        for i in indices:
            actual = call("engine.execute", command="cell.get", params={"cell": f"G{i + 2}"})
            expr, kind, expected = states[state][i]
            assert actual["type"] == {"n": "number", "e": "error"}[kind], (i, actual)
            if kind == "e":
                assert actual["text"] == expected, (i, actual, expected)
            else:
                assert equal(expr, actual["value"], expected), (i, actual, expected)

    output = folder / "现金流 重算.xlsx"
    client, session = launch("edit")
    with session as proc:
        client.wait(lambda s: s["perf"]["frame"] > 1, "window opened", proc)
        call = client.call
        call("app.open", path=str(original))
        inspect(call, 0)
        call("ui.cell.click", cell="A1")
        call("ui.edit.text", text="-120")
        call("ui.edit.commit")
        inspect(call, 1)
        for command, state in (("edit.undo", 0), ("edit.redo", 1)):
            call("engine.execute", command=command, params={})
            inspect(call, state)
        call("app.save", path=str(output))
    assert not differences(CALENDAR["cells"](output), states[1])
    client, session = launch("reopen")
    with session as proc:
        client.wait(lambda s: s["perf"]["frame"] > 1, "new window opened", proc)
        client.call("app.open", path=str(output))
        inspect(client.call, 1)
    assert CALENDAR["digest"](original) == source_hash
    return {"changedCells": len(changed), "checkedPerState": len(indices), "states": 5,
            "savedTypedCaches": len(states[1]), "undoRedoAndFreshReopen": True, "originalUnchanged": True}


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "output"):
        p.add_argument(name, type=Path)
    p.add_argument("--desktop", type=Path)
    args = p.parse_args()
    old, new, folder = [path.resolve() for path in (args.baseline, args.candidate, args.output)]
    folder.mkdir(parents=True)
    result = {"binaries": [CALENDAR["digest"](p) for p in (old, new)],
              "excel": {book.stem: exchange(old, new, book.resolve(), folder) for book in sorted(DATA.glob("*.xlsx"))}}
    assert set(result["excel"]) == {"base", "edit"}
    if args.desktop:
        result["native"] = native(args.desktop.resolve(), folder)
    (folder / "report.json").write_text(json.dumps(result, indent=2) + "\n")
