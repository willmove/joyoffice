"""Product-entry regression against the frozen JoyOffice stages 0 and 1 binaries."""
import argparse
from collections import Counter
import hashlib
import json
import math
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
from xml.etree import ElementTree as ET
from zipfile import ZIP_DEFLATED, ZipFile

from check_craft_pilots import APPS, BASE, ROOT, cli, digest, sheet_save, write_json

NS = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"


def formulas():
    groups = {
        "NORM.DIST NORMDIST": ("A1,3,2,TRUE", "-12,3,2,FALSE", "1,3,0,TRUE", '"bad",0,1,TRUE', "1/0,0,1,TRUE"),
        "NORM.INV NORMINV": ("0.975,3,2", "1E-15,3,2", "0.999999999999999,3,2", "0,3,2", "1,3,2"),
        "NORM.S.DIST": ("A1,TRUE", "-10,TRUE", "3,FALSE", "-37,TRUE", "3,1"),
        "NORMSDIST": ("A1", "-10", "-37"),
        "NORM.S.INV NORMSINV": ("0.5", "1E-25", "0.999999999999999", "0", "1"),
        "T.DIST": ("-2,9.8,TRUE", "2,9,FALSE", "0,1,TRUE", "0,0,TRUE", "-100,20,TRUE"),
        "T.DIST.RT": ("2,9", "1E10,2", "-2,9", "1,0", "0,1", "1E-10,4"),
        "T.DIST.2T": ("2,9", "1E8,4", "-1,3", "0,1"),
        "TDIST": ("2,9.8,1", "2,9,2.2", "1,2,0", "-1,2,2"),
        "T.INV": ("0.99,5", "0.01,5", "0.5,100", "0.50000000000001,1", "1E-30,4", "0,2"),
        "T.INV.2T TINV": ("0.05,12", "1,9", "1E-20,1", "0,9"),
        "T.TEST TTEST": ("A1:A5,B1:B5,2,1", "A1:A5,B1:B5,1,2", "A1:A5,B1:B5,2,3", "A1:A5,B1:B5,3,2", "A1:A5,A1:A5,1,1"),
        "CHISQ.DIST": ("3,4,TRUE", "3,4,FALSE", "0,1,FALSE", "0,2,FALSE", "0,3,FALSE", "-1,4,TRUE", "1,0,TRUE", "1,10000000001,TRUE"),
        "CHISQ.DIST.RT CHIDIST": ("3,4", "100,4", "0,4", "-2,1", "9,5.8"),
        "CHISQ.INV": ("0.5,4", "0,4", "1,4", "1E-25,1", "0.999999999999999,3"),
        "CHISQ.INV.RT CHIINV": ("0.5,4", "1,4", "0,4", "1E-40,5", "0.999999999999999,1"),
        "CHISQ.TEST CHITEST": ("A1:A5,B1:B5", "A1:B2,A3:B4", "A1:A4,B1:B5", "A1:A3,C1:C3"),
        "F.DIST": ("2,4,9,TRUE", "2,4,9,FALSE", "0,1,3,FALSE", "0,2,9,FALSE", "-3,4,9,TRUE", "1E200,2,7,TRUE", "0,3,4,FALSE"),
        "F.DIST.RT FDIST": ("2,4.8,9.9", "1E15,4,3", "0,4,9", "-1,4,9", "2,0,4"),
        "F.INV": ("0.9,9,10", "0,2,7", "1,2,7", "1E-30,2,3", "0.999999999999999,2,6"),
        "F.INV.RT FINV": ("0.1,9,10", "1,9,10", "0,9,10", "1E-30,7,4"),
        "F.TEST FTEST": ("A1:A5,B1:B5", "A1:A5,C1:C3"),
        "BINOM.DIST BINOMDIST": ("3,9,0.4,TRUE", "3,9,0.4,FALSE", "3.9,9.8,0.4,TRUE", "5,4,0.5,TRUE", "0,7,0,FALSE",
                                  "6,7,1,TRUE", "0,100000,1E-18,TRUE", "0,100000,1E-18,FALSE", "100,1000,0.5,TRUE", "2000,10000,0.25,TRUE",
                                  "4000,10001,0.4,TRUE", "0,0,1,FALSE", "1,3,-0.1,TRUE"),
        "BINOM.DIST.RANGE": ("100,0.3,25,35", "10,0.3,3", "10,0.3,2,1", "10,0,0,2", "10,1,8,10", "10000,0.5,5800,5810"),
        "BINOM.INV CRITBINOM": ("11,0.3,0.75", "100000,0.3,0.501", "1,0.5,0.5", "4,0.5,0", "4,0.5,1", "0,0.4,0.5", "5,1,0.5", "5,0,0.5"),
        "POISSON.DIST POISSON": ("4,2.2,TRUE", "4,2.2,FALSE", "0,0,TRUE", "0,0,FALSE", "1,0,FALSE", "-1,2,TRUE",
                                  "1000,1000,TRUE", "1001,1000,TRUE", "1,300,TRUE", "1,300,FALSE", "7.9,4,TRUE"),
        "EXPON.DIST EXPONDIST": ("A1,0.15,TRUE", "A1,0.15,FALSE", "1E-18,0.2,TRUE", "0,4,FALSE", "-1,4,TRUE", "1,0,TRUE"),
        "GAMMA": ("4.5", "-0.5", "-1", "0", "170", "200"),
        "GAMMA.DIST GAMMADIST": ("6,3,2,TRUE", "6,3,2,FALSE", "0,0.5,2,FALSE", "0,1,2,FALSE", "0,3,2,FALSE", "-1,4,1,TRUE", "1,0,2,TRUE"),
        "GAMMA.INV GAMMAINV": ("0.3,2,3", "0,3,2", "1,3,2", "1E-30,0.5,1", "0.999999999999999,3,2"),
        "GAMMALN GAMMALN.PRECISE": ("4.5", "0", "-1", "100"),
        "BETA.DIST": ("0.3,2,5,TRUE", "0.3,2,5,FALSE", "4,2,3,TRUE,1,6", "4,2,3,FALSE,1,6", "0,0.5,2,FALSE",
                     "0,1,2,FALSE", "1,2,1,FALSE", "1,2,0.5,FALSE", "-1,2,3,TRUE", "0.5,2,3,TRUE,1,0"),
        "BETADIST": ("0.3,2,5", "4,2,3,1,6"),
        "BETA.INV BETAINV": ("0.4,2,3,1,6", "0,1,2", "1,1,2", "1E-25,0.5,3", "0.999999999999999,2,0.5", "0.2,0,1"),
        "LOGNORM.DIST": ("2,0.5,1.2,TRUE", "2,0.5,1.2,FALSE", "0,0,2,TRUE", "1,0,0,TRUE"),
        "LOGNORMDIST": ("2,0.5,1.2", "0,0,2"),
        "LOGNORM.INV LOGINV": ("0.97,0.5,1.2", "0,0,2", "1,0,2", "0.99,0,0"),
        "WEIBULL.DIST WEIBULL": ("2,3,5,TRUE", "2,3,5,FALSE", "0,1,5,FALSE", "0,0.5,5,FALSE", "0,2,5,FALSE",
                                  "0,2,5,TRUE", "1,2,0,TRUE", "-1,2,5,TRUE", "1E200,3,5,FALSE"),
        "HYPGEOM.DIST": ("3,9,10,30,TRUE", "3,9,10,30,FALSE", "8,9,10,30,TRUE", "3,9,10,10,TRUE",
                         "9,9,10,10,TRUE", "2,4,0,20,TRUE", "0,0,3,20,TRUE", "100,500,600,2000,TRUE",
                         "6.9,9.1,10.7,30.5,FALSE", "9,9,10,30,TRUE"),
        "HYPGEOMDIST": ("3,9,10,30", "2,4,0,20"),
        "NEGBINOM.DIST": ("5,4,0.3,TRUE", "5,4,0.3,FALSE", "0,1,0,FALSE", "0,3,1,FALSE",
                          "0,3,1,TRUE", "5,3,0,TRUE", "5,0,0.3,TRUE", "5.9,4.5,0.3,TRUE"),
        "NEGBINOMDIST": ("5,4,0.3", "5,0,0.3"),
        "CONFIDENCE CONFIDENCE.NORM": ("0.05,2,6.9", "1E-20,2,6", "0,2,6", "0.05,2,0", "0.05,2,1"),
        "CONFIDENCE.T": ("0.05,2,6.9", "0.05,2,1", "1E-25,2,6", "0,2,6"),
        "Z.TEST ZTEST": ("A1:A5,3", "A1:A5,-1,0.2", "A1:A5,3,0", "C1:C3,0"),
        "FISHER": ("0.2", "-0.8", "1", "-1", "1E-18"),
        "FISHERINV": ("-3", "0", "0.5"),
        "PHI GAUSS": ("0", "3", "-3", "1E-18", "-10"),
    }
    return [f"{name}({args})" for names, arguments in groups.items() for name in names.split() for args in arguments]


def sheet_fixture(destination):
    template = ROOT / "scripts/joysheet/tests/data/engineering-depreciation/base.xlsx"
    worksheet = ET.Element(f"{{{NS}}}worksheet")
    data = ET.SubElement(worksheet, f"{{{NS}}}sheetData")
    for i, values in enumerate(((6, 3, 0), (2, 2, 0), (4, 7, 0), (3, 4), (8, 9)), 1):
        row = ET.SubElement(data, f"{{{NS}}}row", r=str(i))
        for column, value in zip("ABC", values):
            cell = ET.SubElement(row, f"{{{NS}}}c", r=f"{column}{i}")
            ET.SubElement(cell, f"{{{NS}}}v").text = str(value)
    rows = list(data)
    for i, formula in enumerate(formulas(), 2):
        row = rows[i - 1] if i <= len(rows) else ET.SubElement(data, f"{{{NS}}}row", r=str(i))
        cell = ET.SubElement(row, f"{{{NS}}}c", r=f"G{i}")
        name = formula.split("(", 1)[0]
        prefix = "_xlfn." if "." in name or name in {"GAMMA", "PHI", "GAUSS"} else ""
        ET.SubElement(cell, f"{{{NS}}}f").text = prefix + formula
        ET.SubElement(cell, f"{{{NS}}}v").text = "0"
    with ZipFile(template) as source, ZipFile(destination, "w", ZIP_DEFLATED) as target:
        for name in source.namelist():
            target.writestr(name, ET.tostring(worksheet, encoding="utf-8", xml_declaration=True)
                            if name == "xl/worksheets/sheet1.xml" else source.read(name))


def sheet_comparison(old, new, source, reference, out):
    work = runpy.run_path(str(ROOT / "scripts/joysheet/formula_workbook.py"))
    original = digest(source)
    expressions = formulas()

    def normalized(formula):
        # Writers can spell the same binary64 literal in decimal or exponent notation.
        return re.sub(r"(?<![A-Z0-9_.])\d+(?:\.\d+)?(?:E[+-]?\d+)?",
                      lambda token: repr(float(token.group())), formula.upper())

    def values(book):
        rows = work["cells"](book)
        assert [normalized(expr) for expr, _, _ in rows] == [normalized(expr) for expr in expressions], book
        return [(kind, value) for _, kind, value in rows]

    def compare(actual, expected):
        assert len(actual) == len(expected) == len(formulas())
        differences = []
        for row, (expr, got, wanted) in enumerate(zip(formulas(), actual, expected), 2):
            if got == wanted:
                continue
            if got[0] == wanted[0] == "n" and math.isclose(float(got[1]), float(wanted[1]), rel_tol=1e-10):
                continue
            if expr == "T.INV(0.50000000000001,1)" and got[0] == wanted[0] == "n":
                if abs(float(got[1]) - float(wanted[1])) <= 5e-17:
                    continue
            differences.append({"cell": f"G{row}", "formula": expr, "actual": got, "expected": wanted})
        return differences

    results = {}
    for label, build in (("baseline", old), ("candidate", new)):
        path = out / f"probability-{label}.xlsx"
        sheet_save(build, source, path)
        results[label] = values(path)
        for writer in ("baseline", "candidate") if label == "candidate" else ("baseline",):
            writer_path = out / f"probability-{writer}.xlsx"
            reopen = out / f"probability-{writer}-to-{label}.xlsx"
            sheet_save(build, writer_path, reopen)
            assert not compare(values(reopen), results[label])
    reopen = out / "probability-candidate-to-baseline.xlsx"
    sheet_save(old, out / "probability-candidate.xlsx", reopen)
    assert not compare(values(reopen), results["baseline"])
    assert original == digest(source)
    excel = values(reference)
    changed = compare(results["candidate"], results["baseline"])
    before = compare(results["baseline"], excel)
    after = compare(results["candidate"], excel)
    changed_cells = {row["cell"] for row in changed}
    remaining_cells = {row["cell"] for row in after}
    assert not changed_cells & remaining_cells
    assert changed_cells | remaining_cells == {row["cell"] for row in before}
    return {
        "sourceSha256": original,
        "excelSha256": digest(reference),
        "cases": len(formulas()),
        "byFunction": dict(Counter(f.split("(", 1)[0] for f in formulas())),
        "candidateVsBaseline": changed,
        "baselineVsExcel": before,
        "candidateVsExcel": after,
        "crossVersionReadbacks": 4,
    }


def legacy_presets():
    path = "crates/joydeck-legacy/src/model/preset/vendor/catalog.rs"
    source = subprocess.check_output(["git", "show", f"{BASE}:{path}"], cwd=ROOT).decode("utf-8")
    section = source.split("pub static CATALOG", 1)[1].split("pub fn info", 1)[0]
    pattern = r'p!\(\s*"([^"]+)",\s*"(?:[^"\\]|\\.)*",\s*\w+,\s*\[([^\]]*)\]\s*,?\s*\)'
    presets = {name: [int(v) for v in values.split(",") if v.strip()] for name, values in re.findall(pattern, section)}
    assert len(presets) == 183
    return presets


def deck_fixtures(folder):
    base = ROOT / "tests/fixtures/joydeck/r6-r7/legacy-1.11.joydeck"
    folder.mkdir(exist_ok=True)
    cases = {}
    with ZipFile(base) as z:
        entries = {name: z.read(name) for name in z.namelist()}
    document = json.loads(entries["document.json"])
    saved_slide = document["slides"][0]
    saved_object = saved_slide["objects"][0]
    presets = legacy_presets()
    objects = []
    for shape, defaults in presets.items():
        for adjustment in ([], [v + 17 for v in defaults], defaults[:1] if len(defaults) > 1 else []):
            if not adjustment and any(o["preset"]["name"] == shape for o in objects):
                continue
            i = len(objects)
            objects.append(dict(saved_object, id=f"object-2-{i + 2}", x=15.0 + 152 * (i % 6),
                                y=15.0 + 100 * (i // 6 % 5), width=132.0, height=80.0,
                                strokeWidth=1.5, rotation=float(i % 4), preset={
                                    "name": shape, "no_fill": bool(i % 2), "adjustments": adjustment,
                                }))
    main = dict(document, slides=[
        dict(saved_slide, id=f"slide-1-{offset // 30 + 2}", objects=objects[offset:offset + 30])
        for offset in range(0, len(objects), 30)
    ])
    cases["preset-catalog"] = main
    for name, preset in {
        "unknown-name": {"name": "not-A-Preset"},
        "extra-adjustment": {"name": "triangle", "adjustments": [100, 200]},
        "noninteger-adjustment": {"name": "triangle", "adjustments": [50.25]},
        "over-budget-adjustment": {"name": "triangle", "adjustments": [1000000001]},
    }.items():
        cases[name] = dict(document, slides=[dict(saved_slide, objects=[dict(saved_object, preset=preset)])])
    for name, data in cases.items():
        path = folder / (name + ".joydeck")
        if not path.exists():
            with ZipFile(path, "w", ZIP_DEFLATED) as z:
                for part, content in entries.items():
                    z.writestr(part, json.dumps(data, ensure_ascii=False).encode("utf-8")
                               if part == "document.json" else content)
    return len(objects)


def deck_comparison(old, new, folder, out):
    encrypted = folder / "powerpoint16-encrypted.pptx"
    encrypted_hash = digest(encrypted)
    for label in ("EncryptionInfo", "EncryptedPackage"):
        assert label.encode("utf-16-le") in encrypted.read_bytes()
    assert encrypted.read_bytes()[:8] == bytes.fromhex("d0cf11e0a1b11ae1")

    def run(build, *args, success=True):
        p = subprocess.run([cli(build, "deck"), *args], capture_output=True, encoding="utf-8", timeout=120)
        assert (p.returncode == 0) == success, (list(map(str, args)), p.stderr)
        return p.stdout if success else (p.returncode, p.stderr)

    old_result = run(old, "inspect", encrypted, success=False)
    assert old_result == run(new, "inspect", encrypted, success=False)
    assert "encrypted Office files are not supported" in old_result[1]
    assert encrypted_hash == digest(encrypted)
    fixtures = folder / "legacy"
    objects = deck_fixtures(fixtures)
    rejected = {}
    positive = [fixtures / "preset-catalog.joydeck",
                ROOT / "tests/fixtures/joydeck/r6-r7/legacy-1.11.joydeck",
                ROOT / "tests/fixtures/joydeck/r6-r7/legacy-1.0.joydeck",
                ROOT / "tests/fixtures/joydeck/r9-ui2/legacy-review-media.joydeck"]
    for path in sorted(fixtures.glob("*.joydeck")):
        if path in positive:
            continue
        original = digest(path)
        destination = out / (path.stem + "-must-not-exist.joydeck")
        expected = run(old, "migrate", path, "--output", destination, success=False)
        assert expected == run(new, "migrate", path, "--output", destination, success=False)
        assert not destination.exists() and digest(path) == original
        rejected[path.stem] = {"sourceSha256": original, "exit": expected[0], "error": expected[1].strip()}
    reports = {}
    for path in positive:
        original = digest(path)
        snapshots = {}
        for label, build in (("baseline", old), ("candidate", new)):
            location = out / label / path.stem
            location.mkdir(parents=True)
            migrated = location / "migrated.joydeck"
            run(build, "migrate", path, "--output", migrated)
            inspect = json.loads(run(build, "inspect", migrated))
            content = json.loads(run(build, "content", migrated))
            # Either binary must be able to open the other binary's v2 result.
            other = new if label == "baseline" else old
            assert inspect == json.loads(run(other, "inspect", migrated))
            # The byte count describes this generated container. Each reader
            # must report its actual size before comparing document semantics.
            assert inspect.pop("source_bytes") == migrated.stat().st_size
            with ZipFile(migrated) as archive:
                report = json.loads(archive.read("migration/report.json"))
                assert archive.read("migration/legacy-original.joydeck") == path.read_bytes()
            exports = {}
            for mode in ("preserve", "regenerate"):
                pp = location / (mode + ".pptx")
                run(build, "exchange", migrated, "--mode", mode, "--output", pp)
                with ZipFile(pp) as archive:
                    exports[mode] = {part: hashlib.sha256(archive.read(part)).hexdigest()
                                     for part in archive.namelist() if part.startswith("ppt/slides/")}
            images = location / "pages"
            if path.stem == "preset-catalog":
                run(build, "render", migrated, "--all", "--output", images, "--scale", "0.5")
            pixels = {p.name: digest(p) for p in images.glob("*.png")}
            edit_readback = None
            if path.stem == "legacy-1.11":
                shape = content["slides"][0]["shapes"][0]
                edit_readback = {}
                commands = [
                    {"command": "select", "page": 1, "shapes": [shape["id"]]},
                    {"command": "content", "action": {"kind": "shape-preset", "name": "ellipse"}},
                ]
                for operation, expected in (("edit", "ellipse"), ("undo", "triangle")):
                    script = location / f"{operation}.json"
                    changed = location / f"{operation}.joydeck"
                    write_json(script, commands if operation == "edit" else commands + [{"command": "undo"}])
                    run(build, "edit", migrated, "--script", script, "--output", changed)
                    reread = json.loads(run(other, "content", changed))["slides"][0]["shapes"][0]
                    assert reread["id"] == shape["id"] and reread["geom"]["name"] == expected
                    if operation == "undo":
                        assert reread["geom"] == shape["geom"]
                    edit_readback[operation] = reread["geom"]
            snapshots[label] = (inspect, report, content, exports, pixels, edit_readback)
        assert digest(path) == original
        assert snapshots["baseline"] == snapshots["candidate"], path
        inspect, report, _, exports, pixels, edit_readback = snapshots["candidate"]
        reports[path.stem] = {
            "sourceSha256": original, "slides": len(inspect["slides"]),
            "sourceIds": sum(len(o) for _, _, o in report["source_ids"]), "preserveSlideParts": len(exports["preserve"]),
            "regenerateSlideParts": len(exports["regenerate"]), "exactRenderedPngs": len(pixels),
            **({"crossVersionEditAndUndo": edit_readback} if edit_readback else {}),
        }
    return {"powerPoint16": {"sourceSha256": encrypted_hash, "exit": old_result[0], "message": old_result[1].strip()},
            "legacyPresetNames": 183, "presetObjects": objects, "positive": reports, "rejected": rejected}



def word_comparison(old, new, folder, out):
    from functools import partial
    from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
    from threading import Thread

    release = folder / "release"
    proof = runpy.run_path(str(ROOT / "scripts/joyword/check-spelling-order.py"))
    publication = runpy.run_path(str(ROOT / "scripts/joyword/check-pack-publication.py"))
    raw_index = (release / "index.json").read_bytes()
    assert hashlib.sha256(raw_index).hexdigest() == proof["PUBLISHED_INDEX"]
    current = json.loads(raw_index)
    dutch = next(p for p in current["packs"] if p["locale"] == "nl")
    archive = (release / "nl.tar.zst").read_bytes()
    assert hashlib.sha256(archive).hexdigest() == dutch["sha256"]
    mode = "redirect"
    requests = Counter()

    class Server(SimpleHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_GET(self):
            requests[self.path] += 1
            if not self.path.startswith("/probe/"):
                return super().do_GET()
            if self.path == "/probe/index.json":
                pack = dict(dutch, url=f"{base}/probe/archive.tar.zst")
                return self.body(json.dumps(dict(current, packs=[pack])).encode("utf-8"))
            if self.path == "/probe/archive.tar.zst":
                if mode == "redirect":
                    self.send_response(302)
                    self.send_header("Location", "/nl.tar.zst")
                    self.end_headers()
                elif mode == "oversize":
                    self.body(archive + b"!")
                elif mode == "digest":
                    self.body(archive[:-1] + bytes([archive[-1] ^ 1]))
                else:
                    self.send_error(404)
                return
            self.send_error(404)

        def body(self, body):
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    server = ThreadingHTTPServer(("127.0.0.1", 0), partial(Server, directory=release))
    base = f"http://127.0.0.1:{server.server_port}"
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        working = out / "language-packs"
        working.mkdir()
        source = working / "proof.docx"
        proof["make_document"](source, "nl", ["lettergrepen", "teh"])
        original = digest(source)
        readings = {}
        refresh = 'proof.packs={"refresh":true}'
        install = 'proof.installPack={"locale":"nl"}'
        remove = 'proof.removePack={"locale":"nl"}'
        for label, build, reader in (("baseline", old, new), ("candidate", new, old)):
            store = working / label
            store.mkdir()
            inventory = store / "language-packs.json"
            settings = store / "settings.json"
            url = base + "/probe/index.json"
            env = dict(os.environ, CRAFT_FONT_STORE=str(store), CRAFT_LANGUAGE_PACK_INDEX=url,
                       JOYWORD_CONFIG_DIR=str(working / f"{label}-config"))

            def run(binary, *operations, success=True):
                p = subprocess.run([cli(binary, "word"), "run", source,
                                    *(x for cmd in operations for x in ("--cmd", cmd))],
                                   capture_output=True, env=env, encoding="utf-8", timeout=90)
                assert (p.returncode == 0) == success, p.stderr
                return p.stderr.strip()

            before = requests[url.removeprefix(base)]
            consent = run(build, refresh, success=False)
            assert requests[url.removeprefix(base)] == before
            assert not (store / "indexes" / "language-packs.json").exists()
            saved_settings = b'{"network":"on","unrelatedFutureKey":42}'
            settings.write_bytes(saved_settings)
            run(build, refresh)
            errors = {}
            for mode in ("digest", "oversize", "unavailable"):
                errors[mode] = run(build, install, success=False)
                assert not (store / "language-packs" / "nl").exists()
                assert not inventory.exists()
            mode = "redirect"
            run(build, install)
            run(reader, "proof.packs")
            assert settings.read_bytes() == saved_settings
            valid = json.loads(inventory.read_bytes())
            assert len(valid["packs"]) == 1 and valid["packs"][0]["sha256"] == dutch["sha256"]
            orphan = store / "language-packs" / "zz"
            orphan.mkdir()
            (orphan / "pack.toml").write_text('name="Inventory recovery"\nlicence="CC0-1.0"\n')

            def recover(binary):
                # The inventory is read by the locked publisher and journal
                # recovery. Listing an otherwise settled store scans labels.
                journal = store / ".language-pack-change"
                journal.mkdir()
                (journal / "previous.json").write_text(json.dumps({
                    "locale": "nl", "had_folder": True, "entry": valid["packs"][0],
                }))
                run(binary, "proof.packs")
                assert not journal.exists()

            malformed = b'{"packs": invalid json'
            inventory.write_bytes(malformed)
            recover(build)
            assert (store / "language-packs.json.bad").read_bytes() == malformed
            repaired = inventory.read_bytes()
            (store / "language-packs.json").unlink()
            recover(reader)
            assert inventory.read_bytes() == repaired
            oversized = b"!" * ((4 << 20) + 1)
            inventory.write_bytes(oversized)
            recover(build)
            assert (store / "language-packs.json.bad").read_bytes() == oversized
            assert inventory.read_bytes() == repaired
            fallback = next(p for p in json.loads(repaired)["packs"] if p["locale"] == "zz")
            assert fallback["name"] == "Inventory recovery" and fallback["licence"] == "CC0-1.0"
            run(reader, remove)
            assert json.loads(inventory.read_bytes())["packs"] == [fallback]
            readings[label] = {"consent": consent, "errors": errors, "restored": fallback}
        assert readings["baseline"] == readings["candidate"], readings
        assert digest(source) == original
        result = {"consentDigestSizeUnavailable": readings["candidate"],
                  "crossVersionInventoryRecovery": True, "oversizedInventoryRetained": True,
                  "sourceSha256": original}

        all_packs = working / "official"
        all_packs.mkdir()
        result["publishedArchives"] = publication["verify"](
            cli(old, "word"), cli(new, "word"), release, all_packs, archive_base=base)
        assert result["publishedArchives"]["archives"] == len(current["packs"])
        return result
    finally:
        server.shutdown()
        thread.join()
        server.server_close()



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "compare"))
    parser.add_argument("folder", type=Path)
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    folder = args.folder.resolve()
    expected = folder / "probability-excel16.xlsx"
    source = folder / "probability.xlsx"
    if args.action == "prepare":
        folder.mkdir(exist_ok=True, parents=True)
        if not source.exists():
            sheet_fixture(source)
        if not expected.exists():
            shutil.copyfile(source, expected)
        deck_fixtures(folder / "legacy")
    else:
        if not (args.frozen and args.candidate and args.output):
            parser.error("compare requires --frozen, --candidate and --output")
        frozen = args.frozen.resolve()
        old, new, out = frozen / BASE, args.candidate.resolve(), args.output.resolve()
        manifest = json.loads((frozen / "baseline.json").read_bytes())
        assert manifest["binaries"] == {app: digest(old / f"{app}.exe") for app in APPS}
        assert manifest["cargoLockSha256"] == digest(old / "Cargo.lock")
        out.mkdir(exist_ok=False, parents=True)
        write_json(out / "sheet.json", sheet_comparison(old, new, source, expected, out))
        write_json(out / "deck.json", deck_comparison(old, new, folder, out))
        write_json(out / "word.json", word_comparison(old, new, folder, out))


if __name__ == "__main__":
    main()
