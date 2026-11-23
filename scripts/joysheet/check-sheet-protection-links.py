"""Compare worksheet protection, legacy password hashes, and hyperlinks across XLS, XLSB, and XLSX."""

import argparse
import json
import runpy
import subprocess
import sys
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from native_pilot_support import SheetControl, app, digest, port

WORKBOOK = runpy.run_path(str(Path(__file__).with_name("check-workbook-view.py")))
DATA = Path(__file__).resolve().parent / "tests/data/sheet-protection-links"

PROTECTED_SPEC = {
    "protected": True,
    "password": "83AF",
    "selectLocked": False,
    "selectUnlocked": True,
    "formatCells": True,
    "formatColumns": False,
    "formatRows": False,
    "insertColumns": False,
    "insertRows": False,
    "deleteColumns": False,
    "deleteRows": False,
    "sort": True,
    "autoFilter": True,
}

OPEN_SPEC = {
    "protected": False,
}

PROTECTED_LINKS = {
    "A1": {"target": "https://example.org/spec#sec", "tooltip": "Spec Link", "external": True},
    "B1": {"target": "https://example.org/spec#sec", "tooltip": "Spec Link", "external": True},
    "A2": {"target": "Notes!B2", "tooltip": None, "external": False},
}

OPEN_LINKS = {
    "A1": {"target": "https://example.org/home", "tooltip": "Home", "external": True},
    "A2": {"target": "Notes!A1", "tooltip": None, "external": False},
}

CASES = {
    "protected_links.xls": (PROTECTED_SPEC, {}),
    "protected_links.xlsb": (PROTECTED_SPEC, PROTECTED_LINKS),
    "protected_links.xlsx": (PROTECTED_SPEC, PROTECTED_LINKS),
    "open_links.xls": (OPEN_SPEC, {}),
    "open_links.xlsb": (OPEN_SPEC, OPEN_LINKS),
    "open_links.xlsx": (OPEN_SPEC, OPEN_LINKS),
}


def flag(node, attr, default=False):
    val = node.get(attr)
    if val is None:
        return default
    return val not in {"0", "false", "FALSE"}


def all_relations(archive, part):
    parent, name = part.rsplit("/", 1) if "/" in part else ("", part)
    rel_path = f"{parent}/_rels/{name}.rels" if parent else f"_rels/{name}.rels"
    if rel_path not in archive.namelist():
        return {}
    out = {}
    for rel in ET.fromstring(archive.read(rel_path)):
        out[rel.get("Id")] = (rel.get("Target"), rel.get("TargetMode") == "External")
    return out


def inspect_meta(file):
    with ZipFile(file) as archive:
        workbook = ET.fromstring(archive.read("xl/workbook.xml"))
        sheet = workbook.find(".//{*}sheet[@name='Details']")
        _, part = WORKBOOK["relations"](archive, "xl/workbook.xml")[sheet.get(WORKBOOK["REL"])]
        root = ET.fromstring(archive.read(part))
        srels = all_relations(archive, part)
    sp = root.find("{*}sheetProtection")
    if sp is not None and flag(sp, "sheet", False):
        prot = {
            "protected": True,
            "password": sp.get("password"),
            "selectLocked": not flag(sp, "selectLockedCells", False),
            "selectUnlocked": not flag(sp, "selectUnlockedCells", False),
            "formatCells": not flag(sp, "formatCells", True),
            "formatColumns": not flag(sp, "formatColumns", True),
            "formatRows": not flag(sp, "formatRows", True),
            "insertColumns": not flag(sp, "insertColumns", True),
            "insertRows": not flag(sp, "insertRows", True),
            "deleteColumns": not flag(sp, "deleteColumns", True),
            "deleteRows": not flag(sp, "deleteRows", True),
            "sort": not flag(sp, "sort", True),
            "autoFilter": not flag(sp, "autoFilter", True),
        }
    else:
        prot = {"protected": False}

    links = {}
    for hl in root.findall("{*}hyperlinks/{*}hyperlink"):
        ref = hl.get("ref")
        rid = hl.get(WORKBOOK["REL"])
        loc = hl.get("location")
        tip = hl.get("tooltip")
        target = ""
        external = False
        if rid and rid in srels:
            target, _ = srels[rid]
            external = True
        if loc:
            target = f"{target}#{loc}" if target else loc.lstrip("#")
        links[ref] = {"target": target, "tooltip": tip, "external": external}
    return {"protection": prot, "hyperlinks": links}


def cli_resave(cli, source, dest):
    subprocess.run(
        [cli, "run", "--in", source, "--cmd", "review.workbookStatistics={}", "--out", dest, "--quiet"],
        check=True,
        capture_output=True,
        timeout=40,
    )


def cli_cell_set_fails(cli, source, dest):
    proc = subprocess.run(
        [cli, "run", "--in", source, "--cmd", 'cell.set={"cell":"A5","input":"blocked"}', "--out", dest, "--quiet"],
        capture_output=True,
        text=True,
        timeout=40,
    )
    return proc.returncode != 0


def exchange(book, old, new, output):
    exp_prot, exp_links = CASES[book.name]
    saved = {}
    for tag, writer in [("baseline", old), ("candidate", new)]:
        path = output / f"{book.name}-{tag}.xlsx"
        cli_resave(writer, book, path)
        actual = inspect_meta(path)
        if tag == "candidate":
            assert actual["protection"] == exp_prot, (book.name, actual["protection"], exp_prot)
            assert actual["hyperlinks"] == exp_links, (book.name, actual["hyperlinks"], exp_links)
        saved[tag] = actual
        for reader_name, reader in [("baseline", old), ("candidate", new)]:
            reopened = output / f"{book.name}-{tag}-{reader_name}.xlsx"
            cli_resave(reader, path, reopened)
            if tag == reader_name == "candidate":
                assert inspect_meta(reopened) == actual, (book.name, inspect_meta(reopened), actual)
    blocked_edit = cli_cell_set_fails(new, book, output / f"{book.name}-blocked.xlsx")
    assert blocked_edit is exp_prot["protected"], (book.name, blocked_edit, exp_prot["protected"])
    return {
        "sourceSha256": digest(book),
        "writerMeta": saved,
        "candidateBlocksProtectedEdit": blocked_edit,
        "readerWriterPairs": 4,
    }


def native(program, books, root):
    root.mkdir(parents=True, exist_ok=True)
    logs = {}
    added_link = {"target": "https://example.org/added", "tooltip": "Added", "external": True}
    for phase in ("edit", "reopen"):
        endpoint, profile = port(), root / (phase + "-profile")
        control = SheetControl(endpoint, profile)
        with app(
            program,
            ["--control", str(endpoint), "--diagnostics"],
            root,
            {"JOYSHEET_CONFIG_DIR": str(profile)},
            phase + ".log",
            unset=("GRIDCRAFT_CONTROL_TOKEN", "JOYSHEET_CONTROL_TOKEN"),
        ) as proc:
            control.wait(lambda s: s["perf"]["frame"] > 1, "native grid", proc)
            call = control.call
            for book in books:
                exp_prot, exp_links = CASES[book.name]
                dest = root / ("保护与链接-" + book.name + ".xlsx")
                call("app.open", path=str(book if phase == "edit" else dest))
                call("ui.awaitIdle")
                if phase == "reopen":
                    reopen_save = root / ("重开-" + book.name + ".xlsx")
                    call("app.save", path=str(reopen_save))
                    after = inspect_meta(reopen_save)
                    if exp_prot["protected"]:
                        assert not after["protection"]["protected"], (book.name, after)
                    else:
                        assert after["protection"]["protected"] and after["protection"]["formatCells"] and after["protection"]["sort"], (book.name, after)
                    assert after["hyperlinks"].get("C2") == added_link, (book.name, after["hyperlinks"])
                    for k, v in exp_links.items():
                        assert after["hyperlinks"].get(k) == v, (book.name, k, after["hyperlinks"])
                    continue

                if exp_prot["protected"]:
                    # Wrong password must fail before unprotecting with the real legacy password.
                    wrong_rejected = False
                    try:
                        call("engine.execute", command="review.unprotectSheet", params={"password": "wrong"})
                    except Exception:
                        wrong_rejected = True
                    assert wrong_rejected, (book.name, "wrong password was not rejected")
                    call("engine.execute", command="review.unprotectSheet", params={"password": "password"})
                    call("engine.execute", command="insert.link", params={"cell": "C2", "target": added_link["target"], "tooltip": added_link["tooltip"]})
                    call("ui.awaitIdle")
                    call("engine.execute", command="edit.undo", params={})
                    call("engine.execute", command="edit.undo", params={})
                    call("ui.awaitIdle")
                    undo_dest = root / ("撤销-" + book.name + ".xlsx")
                    call("app.save", path=str(undo_dest))
                    undone = inspect_meta(undo_dest)
                    assert undone["protection"] == exp_prot and undone["hyperlinks"] == exp_links, (book.name, undone)
                    call("engine.execute", command="edit.redo", params={})
                    call("engine.execute", command="edit.redo", params={})
                    call("ui.awaitIdle")
                    call("app.save", path=str(dest))
                    saved = inspect_meta(dest)
                    assert not saved["protection"]["protected"] and saved["hyperlinks"].get("C2") == added_link, (book.name, saved)
                else:
                    call("engine.execute", command="insert.link", params={"cell": "C2", "target": added_link["target"], "tooltip": added_link["tooltip"]})
                    call("engine.execute", command="review.protectSheet", params={"formatCells": True, "sort": True})
                    call("ui.awaitIdle")
                    call("engine.execute", command="edit.undo", params={})
                    call("engine.execute", command="edit.undo", params={})
                    call("ui.awaitIdle")
                    undo_dest = root / ("撤销-" + book.name + ".xlsx")
                    call("app.save", path=str(undo_dest))
                    undone = inspect_meta(undo_dest)
                    assert undone["protection"] == exp_prot and undone["hyperlinks"] == exp_links, (book.name, undone)
                    call("engine.execute", command="edit.redo", params={})
                    call("engine.execute", command="edit.redo", params={})
                    call("ui.awaitIdle")
                    call("app.save", path=str(dest))
                    saved = inspect_meta(dest)
                    assert saved["protection"]["protected"] and saved["protection"]["formatCells"] and saved["protection"]["sort"], (book.name, saved)
                    assert saved["hyperlinks"].get("C2") == added_link, (book.name, saved)
            logs[phase] = str(root / (phase + ".log"))
    return {"books": [b.name for b in books], "logs": logs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("old", type=Path)
    parser.add_argument("new", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--desktop", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    books = [DATA / name for name in sorted(CASES)]
    before = {b.name: digest(b) for b in books}
    result = {
        "cliSha256": [digest(args.old), digest(args.new)],
        "workbooks": {b.name: exchange(b, args.old, args.new, args.output) for b in books},
    }
    if args.desktop:
        result["native"] = native(args.desktop, books, args.output / "native")
    assert {b.name: digest(b) for b in books} == before
    result["originalsUnchanged"] = True
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
