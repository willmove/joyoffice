"""Compare typed Excel result caches, cross-version saves and edits in the native JoySheet window."""

import argparse
from collections import Counter
import json
from pathlib import Path
import runpy
from xml.etree import ElementTree as ET
from zipfile import ZipFile

CALENDAR = runpy.run_path(str(Path(__file__).with_name("check-workday-calendar.py")))


def cells(source):
    rows = []
    with ZipFile(source) as archive:
        sheet = ET.fromstring(archive.read("xl/worksheets/sheet1.xml"))
        for cell in sheet.iterfind(".//{*}c"):
            address = cell.get("r", "")
            if not address.startswith("G") or not address[1:].isdigit():
                continue
            formula = cell.findtext("{*}f")
            if not formula:
                continue
            assert address == f"G{len(rows) + 2}"
            kind = cell.get("t", "n")
            assert kind in {"n", "e", "str"}, (source, address, kind)
            value = cell.findtext("{*}v")
            assert value is not None or kind == "str"
            rows.append((formula.removeprefix("_xlfn."), kind, value or ""))
    assert rows
    return rows


def differences(rows, reference, number_equal):
    assert len(rows) == len(reference) and rows
    failures = []
    for i, ((f, t, v), (expr, kind, expected)) in enumerate(zip(rows, reference), 2):
        assert f == expr, (i, f, expr)
        if t != kind or (v != expected and (kind != "n" or not number_equal(expr, v, expected))):
            failures.append((f"G{i}", expr, expected, v))
    return failures


def exchange(old, new, source, folder, number_equal):
    digest, save = CALENDAR["digest"], CALENDAR["save"]
    original = digest(source)
    expected = cells(source)
    by_writer = {}
    for label, writer in (("baseline", old), ("candidate", new)):
        output = folder / f"{source.stem}-{label}.xlsx"
        save(writer, source, output, "en")
        by_writer[label] = cells(output)
    errors = {key: differences(rows, expected, number_equal) for key, rows in by_writer.items()}
    assert not errors["candidate"], errors["candidate"]
    # Force each reader to recalculate and save the other writer's formulas.
    for label, reader in (("baseline", old), ("candidate", new)):
        partner = "baseline" if label == "candidate" else "candidate"
        output = folder / f"{source.stem}-{partner}-to-{label}.xlsx"
        save(reader, folder / f"{source.stem}-{partner}.xlsx", output, "en")
        assert not differences(cells(output), by_writer[label], number_equal)
    assert digest(source) == original
    return {
        "sourceSha256": original,
        "independentByFunction": dict(Counter(f.split("(")[0] for f, _, _ in expected)),
        "baselineDifferences": errors["baseline"],
        "candidateDifferences": len(errors["candidate"]),
        "bothGenerationsRecalculatedBothWriters": True,
    }


def native(program, folder, data, new_input, output_name, number_equal):
    original, edited = [data / (tag + ".xlsx") for tag in ("base", "edit")]
    source_hash = CALENDAR["digest"](original)
    states = [cells(book) for book in (original, edited)]
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
            assert actual["type"] == {"n": "number", "e": "error", "str": "text"}[kind], (i, actual)
            if kind != "n":
                assert actual["text"] == expected, (i, actual, expected)
            else:
                assert number_equal(expr, actual["value"], expected), (i, actual, expected)

    output = folder / output_name
    client, session = launch("edit")
    with session as proc:
        client.wait(lambda s: s["perf"]["frame"] > 1, "window opened", proc)
        call = client.call
        call("app.open", path=str(original))
        inspect(call, 0)
        call("ui.cell.click", cell="A1")
        call("ui.edit.text", text=new_input)
        call("ui.edit.commit")
        inspect(call, 1)
        for command, state in (("edit.undo", 0), ("edit.redo", 1)):
            call("engine.execute", command=command, params={})
            inspect(call, state)
        call("app.save", path=str(output))
    assert not differences(cells(output), states[1], number_equal)
    client, session = launch("reopen")
    with session as proc:
        client.wait(lambda s: s["perf"]["frame"] > 1, "new window opened", proc)
        client.call("app.open", path=str(output))
        inspect(client.call, 1)
    assert CALENDAR["digest"](original) == source_hash
    return {"changedCells": len(changed), "checkedPerState": len(indices), "states": 5,
            "savedTypedCaches": len(states[1]), "undoRedoAndFreshReopen": True, "originalUnchanged": True}


def run_family(description, data, new_input, output_name, number_equal):
    p = argparse.ArgumentParser(description=description)
    for name in ("baseline", "candidate", "output"):
        p.add_argument(name, type=Path)
    p.add_argument("--desktop", type=Path)
    args = p.parse_args()
    old, new, folder = [path.resolve() for path in (args.baseline, args.candidate, args.output)]
    folder.mkdir(parents=True)
    result = {"binaries": [CALENDAR["digest"](p) for p in (old, new)],
              "excel": {book.stem: exchange(old, new, book.resolve(), folder, number_equal) for book in sorted(data.glob("*.xlsx"))}}
    assert set(result["excel"]) == {"base", "edit"}
    if args.desktop:
        result["native"] = native(args.desktop.resolve(), folder, data, new_input, output_name, number_equal)
    (folder / "report.json").write_text(json.dumps(result, indent=2) + "\n")
