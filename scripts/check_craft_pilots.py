"""Freeze or compare product-entry results for the three Craft replacement pilots."""
import argparse
import hashlib
import json
import math
import re
import runpy
import struct
import subprocess
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZIP_DEFLATED, ZipFile

ROOT = Path(__file__).resolve().parents[1]
BASE = "14cbb296"
VARIANTS = "third-party/craft-i18n/language-data/en-us/en-variants.txt"
UNITS = "crates/joysheet-functions/src/engineering/units.rs"
FONT = Path("C:/Windows/Fonts/arial.ttf")
CFB_MAGIC = bytes.fromhex("D0CF11E0A1B11AE1")
APPS = ("joyword", "joyword-cli", "joydeck", "joydeck-cli", "joysheet", "joysheet-cli")


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, obj):
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def cli(build, product):
    binary = build / f"joy{product}-cli.exe"
    return binary if binary.is_file() else binary.with_suffix("")


def baseline_source(path):
    return subprocess.check_output(["git", "show", f"{BASE}:{path}"], cwd=ROOT).decode("utf-8")


def word_cases(corpus):
    work = runpy.run_path(str(ROOT / "scripts/joyword/check-spelling-order.py"))
    words = {s for line in baseline_source(VARIANTS).splitlines()
             if line and not line.startswith("#") for s in line.split()[1:]}
    words.update("""colours color's colour's COLOURS COLOR Colour colour’s centred centered
        centring centering organise organizing organising organization organisation cozy cosy
        cosier cozier cosiest coziest cosiness coziness program programme programmes programmed
        honourable honorable analyses analyzes paralyses paralyzes behaviour behavioral favourite
        favorites favorite theatre theater tyres tires manoeuvre maneuver defence defense
        cafés kilometer kilometre catalyzed catalysed encyclopedia encyclopaedia""".split())
    words = sorted(words)
    for locale in ("en-US", "en-CA", "en-GB", "en-AU"):
        for index, start in enumerate(range(0, len(words), 150)):
            folder = corpus / "word" / locale / str(index)
            folder.mkdir(parents=True, exist_ok=True)
            batch = words[start:start + 150]
            source = folder / "sample.docx"
            if not source.is_file():
                work["make_document"](source, locale, batch)
            yield locale, folder, batch


def word_results(build, corpus, encoding="utf-8"):
    work = runpy.run_path(str(ROOT / "scripts/joyword/check-spelling-order.py"))
    return {str(folder.relative_to(corpus)).replace("\\", "/"): {
        "probes": len(words), "issues": work["results"](cli(build, "word"), folder, locale, words, encoding=encoding)
    } for locale, folder, words in word_cases(corpus)}


def compound(stream, sector_size=512, payload=b""):
    """Minimal v3/v4 CFB container with one root stream and one regular data sector."""
    end, free, fat = 0xFFFFFFFE, 0xFFFFFFFF, 0xFFFFFFFD
    data = bytearray(4 * sector_size)
    data[:8] = CFB_MAGIC
    struct.pack_into("<5H", data, 24, 62, 3 if sector_size == 512 else 4,
                     0xFFFE, sector_size.bit_length() - 1, 6)
    struct.pack_into("<9I", data, 40, int(sector_size != 512), 1, 0, 0, 4096,
                     end, 0, end, 0)
    struct.pack_into("<109I", data, 76, 1, *([free] * 108))

    def entry(pos, name, kind, child, start, size):
        utf16 = (name + "\0").encode("utf-16-le")
        data[pos:pos + len(utf16)] = utf16
        struct.pack_into("<HBBIII", data, pos + 64, len(utf16), kind, 1, free, free, child)
        struct.pack_into("<IQ", data, pos + 116, start, size)

    entry(sector_size, "Root Entry", 5, 1, end, 0)
    entry(sector_size + 128, stream, 2, free, 2, len(payload))
    struct.pack_into(f"<{sector_size // 4}I", data, sector_size * 2,
                     end, fat, end, *([free] * (sector_size // 4 - 3)))
    data[sector_size * 3:sector_size * 3 + len(payload)] = payload
    return data


def deck_cases(corpus):
    marker = "EncryptedPackage".encode("utf-16-le")
    for name, content in {
        "v3-package": compound("EncryptedPackage"),
        "v4-info": compound("encryptionINFO", 4096),
        "ordinary-ole": compound("Other"),
        "ordinary-ole-with-marker": compound("Other", payload=marker),
        "truncated-cfb": CFB_MAGIC,
        "truncated-cfb-with-marker": CFB_MAGIC + marker,
        "plain-marker": marker,
    }.items():
        path = corpus / f"{name}.pptx"
        if not path.is_file():
            path.write_bytes(content)
    for name in ("plain", "office-encrypted"):
        assert (corpus / f"{name}.pptx").is_file(), f"missing PowerPoint fixture: {name}"


def deck_results(build, corpus):
    result = {}
    for path in sorted(corpus.glob("*.pptx")):
        proc = subprocess.run([cli(build, "deck"), "inspect", path], capture_output=True, timeout=30)
        message = proc.stderr.decode("utf-8", errors="replace").strip()
        result[path.name] = {"exit": proc.returncode, "error": message}
    return result


def sheet_formulas():
    def convert(number, source, target):
        return f'CONVERT({number},"{source}","{target}")'

    text = baseline_source(UNITS)
    for match in re.finditer(r'u\(&\[(.*?)\],\s*Dim::(\w+)', text, re.S):
        names, dimension = match.groups()
        anchor = {
            "Mass": "g", "Length": "m", "Time": "sec", "Pressure": "Pa", "Force": "N",
            "Energy": "J", "Power": "W", "Magnetism": "T", "Temperature": "K", "Volume": "l",
            "Area": "m2", "Information": "bit", "Speed": "m/s",
        }[dimension]
        for name in re.findall(r'"([^"]+)"', names):
            yield convert(1, name, anchor)
            yield convert(1, anchor, name)
    for prefix in ("Y", "Z", "E", "P", "T", "G", "M", "k", "h", "da", "e", "d", "c",
                   "m", "u", "n", "p", "f", "a", "z", "y", "ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"):
        for unit, target in (("m", "m"), ("m2", "m2"), ("m3", "m3"), ("byte", "bit")):
            yield convert(1, prefix + unit, target)
    for source, target in (("C", "F"), ("F", "C"), ("Rank", "Reau"), ("K", "cel"),
                           ("mK", "K"), ("kK", "fah"), ("F", "fah"), ("Reau", "K")):
        for value in (0, -40, 32, 100, 1e100):
            yield convert(value, source, target)
    for source, target in (("ft", "sec"), ("m^2", "cm^2"), ("in^3", "l"), ("m^3", "m^2"),
                           ("m^2^3", "m2"), ("LBM", "kg"), (" kL", "L"), ("km/h", "m/sec"),
                           ("kiL", "L"), ("kft", "m"), ("s", "sec"), ("min", "sec"), ("GB", "byte"),
                           ("lbf", "N"), ("uk_gal", "L")):
        yield convert(2.5, source, target)
    yield convert('"text"', "m", "in")
    yield convert("1/0", "m", "in")
    yield convert("A1", "in", "cm")
    yield convert("A1", "l", "gal")


def sheet_input(corpus):
    path = corpus / "convert.xlsx"
    if path.is_file():
        return path
    template = ROOT / "scripts/joysheet/tests/data/engineering-depreciation/base.xlsx"
    ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    sheet = ET.Element(f"{{{ns}}}worksheet")
    data = ET.SubElement(sheet, f"{{{ns}}}sheetData")
    row = ET.SubElement(data, f"{{{ns}}}row", r="1")
    input_cell = ET.SubElement(row, f"{{{ns}}}c", r="A1")
    ET.SubElement(input_cell, f"{{{ns}}}v").text = "6"
    for i, formula in enumerate(dict.fromkeys(sheet_formulas()), 2):
        row = ET.SubElement(data, f"{{{ns}}}row", r=str(i))
        cell = ET.SubElement(row, f"{{{ns}}}c", r=f"G{i}")
        ET.SubElement(cell, f"{{{ns}}}f").text = formula
        ET.SubElement(cell, f"{{{ns}}}v").text = "0"
    with ZipFile(template) as old, ZipFile(path, "w", ZIP_DEFLATED) as new:
        for name in old.namelist():
            if name == "xl/worksheets/sheet1.xml":
                new.writestr(name, ET.tostring(sheet, encoding="utf-8", xml_declaration=True))
            else:
                new.writestr(name, old.read(name))
    return path


def sheet_save(build, path, output):
    work = runpy.run_path(str(ROOT / "scripts/joysheet/formula_workbook.py"))
    if output.exists():
        raise FileExistsError(output)
    work["CALENDAR"]["save"](cli(build, "sheet"), path, output, "en")
    return work["cells"](output)


def sheet_diff(actual, expected):
    assert len(actual) == len(expected)
    return [(f"G{i}", list(a), list(b)) for i, (a, b) in enumerate(zip(actual, expected), 2)
            if (a[0], a[1]) != (b[0], b[1]) or (a[2] != b[2] and (
                a[1] != "n" or not math.isclose(float(a[2]), float(b[2]), rel_tol=2e-12)))]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "freeze", "compare"))
    parser.add_argument("folder", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--excel-reference", type=Path)
    args = parser.parse_args()
    if args.action == "compare" and args.candidate is None:
        parser.error("compare requires --candidate")
    if args.excel_reference is not None and not args.excel_reference.is_file():
        parser.error("--excel-reference must name an existing workbook")
    folder = args.folder.resolve()
    corpus, old = folder / "corpus", folder / BASE
    corpus.mkdir(parents=True, exist_ok=True)
    list(word_cases(corpus))
    sheet_input(corpus)
    if args.action == "prepare":
        return
    deck_cases(corpus)
    inputs = [*corpus.glob("*.pptx"), corpus / "convert.xlsx", *corpus.glob("word/*/*/sample.docx")]
    wps = corpus / "convert-excel.xlsx"
    if wps.is_file():
        inputs.append(wps)
    hashes = {file.relative_to(corpus).as_posix(): digest(file) for file in sorted(inputs)}
    if args.action == "freeze":
        comp = {
            "commit": subprocess.check_output(["git", "rev-parse", BASE], cwd=ROOT, text=True).strip(),
            "binaries": {app: digest(old / f"{app}.exe") for app in APPS},
            "cargoLockSha256": digest(old / "Cargo.lock"),
            "font": {"name": FONT.name, "sha256": digest(FONT), "face": 0},
            "variantDataSha256": digest(ROOT / VARIANTS),
            "inputs": hashes,
            "word": word_results(old, corpus),
            "deck": deck_results(old, corpus),
            "sheet": sheet_save(old, corpus / "convert.xlsx", old / "convert-saved.xlsx"),
        }
        write_json(folder / "baseline.json", comp)
        print(f"Frozen {len(hashes)} inputs, six binaries and their results")
        return
    new = args.candidate.resolve()
    reference = json.loads((folder / "baseline.json").read_text(encoding="utf-8"))
    assert hashes == reference["inputs"], "input hashes changed"
    assert reference["binaries"] == {app: digest(old / f"{app}.exe") for app in APPS}
    assert reference["cargoLockSha256"] == digest(old / "Cargo.lock")
    word, deck = word_results(new, corpus), deck_results(new, corpus)
    word_encoding = "utf-8"
    if word != reference["word"]:
        # The original Windows capture used the host ANSI decoder for the UTF-8 pipe.
        assert word_results(old, corpus, "gbk") == reference["word"], "archived Word Review results changed"
        assert word == word_results(old, corpus), "Word Review output changed"
        word_encoding = "utf-8 (frozen binary replayed; archived capture decoded as GBK)"
    assert deck == reference["deck"], (deck, reference["deck"])
    out = args.output.resolve() if args.output else folder / "comparison"
    out.mkdir()
    sheet = sheet_save(new, corpus / "convert.xlsx", out / "new.xlsx")
    sheet_baseline_diff = sheet_diff(sheet, reference["sheet"])
    reopened = {}
    for writer, sample in (("old", old / "convert-saved.xlsx"), ("new", out / "new.xlsx")):
        for tag, reader in (("old", old), ("new", new)):
            got = sheet_save(reader, sample, out / f"{writer}-to-{tag}.xlsx")
            reopened[f"{writer}-to-{tag}"] = len(sheet_diff(got, reference["sheet"]))
    result = {
        "wordProbes": sum(c["probes"] for c in word.values()),
        "wordIssueRows": sum(len(c["issues"]) for c in word.values()),
        "wordResultEncoding": word_encoding,
        "deck": deck,
        "sheetFormulas": len(sheet),
        "sheetBaselineDifferences": sheet_baseline_diff,
        "sheetReopenDifferences": reopened,
    }
    sheet_probes = runpy.run_path(str(ROOT / "scripts/joysheet/formula_workbook.py"))
    for label, path in (("WPS", wps), ("Excel16", args.excel_reference)):
        if path and path.is_file():
            rows = sheet_probes["cells"](path)
            result[label] = {
                "sha256": digest(path),
                "baselineDifferences": sheet_diff(reference["sheet"], rows),
                "candidateDifferences": sheet_diff(sheet, rows),
            }
    write_json(out / "report.json", result)
    assert not sheet_baseline_diff and not any(reopened.values()), result
    print(f'Word {result["wordProbes"]}, Deck {len(deck)}, Sheet {len(sheet)}; baseline equivalent')


if __name__ == "__main__":
    main()
