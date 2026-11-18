"""Check native grammar ranges, missing spaces and published dictionaries in both JoyWord readers."""

import argparse
import json
import runpy
import unicodedata
import xml.etree.ElementTree as ET
from pathlib import Path
from zipfile import ZipFile

SPELL = runpy.run_path(str(Path(__file__).with_name("check-spelling-order.py")))
LANGS = {
    "cs-CZ": ("Škola", "škola", "Opakované slovo"),
    "hr-HR": ("Škola", "škola", "Ponovljena riječ"),
    "id-ID": ("Buku", "buku", "Kata berulang"),
    "it-IT": ("Città", "città", "Parola ripetuta"),
    "pl-PL": ("Szkoła", "szkoła", "Powtórzone słowo"),
    "pt-BR": ("Escola", "escola", "Palavra repetida"),
    "ru-RU": ("Книга", "книга", "Повтор слова"),
    "tr-TR": ("Işık", "ışık", "Yinelenen sözcük"),
    **{
        locale: ("Árbol", "árbol", "Palabra repetida")
        for locale in ("es-ES", "es-MX", "es-419", "es-UY")
    },
}


def grammar(old, new, out):
    results = {}
    for language, (first, second, prefix) in LANGS.items():
        folder = out / language
        folder.mkdir()
        cases = [
            (
                first,
                sep,
                second,
                sep in (" ", "\t") or (sep == "\u00a0" and language != "tr-TR"),
            )
            for sep in (" ", "\t", "\u00a0", ", ", "—", " / ")
        ]
        cases.extend(
            [
                ("123", " ", "123", False),
                ("A1", " ", "a1", False),
                ("Ön", " ", second, False),
            ]
        )
        if language.startswith("es"):
            cases.append((first, " ", unicodedata.normalize("NFD", second), True))
        if language == "tr-TR":
            cases.extend(
                [("Ankara'da", " ", "Ankara'da", True), ("Yavaş", " ", "yavaş", False)]
            )
        if language == "cs-CZ":
            cases.append(("Ano", " ", "ano", False))
        documents = [a + gap + b + "." for a, gap, b, _ in cases]
        before, after = [
            SPELL["results"](cli, folder, language, documents, "grammar")
            for cli in (old, new)
        ]
        assert before == after, (language, before, after)
        repeats = [i for i in after if i["message"].startswith(prefix)]
        expected = []
        for i, (a, gap, b, flag) in enumerate(cases):
            if not flag:
                continue
            second_only = language == "tr-TR"
            begin = len((a + gap).encode()) if second_only else 0
            end = len((a + gap + b).encode())
            fixes = (
                [] if second_only else [a + "-" + b, a] if language == "id-ID" else [a]
            )
            expected.append((i, begin, end, b if second_only else a + gap + b, fixes))
        actual = [
            (
                r["start"]["path"][0],
                r["start"]["off"],
                r["end"]["off"],
                r["text"],
                r["suggestions"],
            )
            for r in repeats
        ]
        assert actual == expected, (language, actual, expected)
        results[language] = {
            "paragraphs": len(cases),
            "expectedRepetitions": len(expected),
            "allGrammarIssues": len(after),
        }
    return results


def missing_spaces(old, new, out):
    # Deliberately give the dictionary both pieces of a split inside a grapheme.
    # The source must stay a unit even if both halves are permitted dictionary entries.
    folder = out / "unicode-split"
    left = "preq"
    accent = "\u0301"
    right = "post"
    words = [left, accent + right, left + accent, right, "blue", "house"]
    SPELL["write_dictionary"](
        folder,
        "nl",
        b"SET UTF-8\n",
        (str(len(words)) + "\n" + "\n".join(words) + "\n").encode(),
    )
    probes = [left + accent + right, "bluehouse"]
    before, after = [
        {
            i["text"]: i["suggestions"]
            for i in SPELL["results"](cli, folder, "nl", probes)
        }
        for cli in (old, new)
    ]
    inp = probes[0]
    invalid = left + " " + accent + right
    valid = left + accent + " " + right
    assert invalid not in after[inp] and valid in after[inp], after
    assert (
        invalid in before[inp]
        and valid in before[inp]
        and "blue house" in before["bluehouse"]
        and "blue house" in after["bluehouse"]
    )
    return {"baseline": before, "candidate": after}


def exchange(old, new, folder, language, replacement):
    cli = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))["run_cli"]
    saved = folder / "校对 副本.docx"
    original = SPELL["sha"](saved)
    binaries = {"baseline": old, "candidate": new}
    readers = {}
    for writer, binary in binaries.items():
        copy = folder / (writer + ".docx")
        cli(binary, "convert", saved, copy)
        with ZipFile(copy) as archive:
            word = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            body = ET.fromstring(archive.read("word/document.xml"))
            assert "".join(t.text or "" for t in body.iter(f"{{{word}}}t")) == replacement
            assert {lang.get(f"{{{word}}}val") for lang in body.iter(f"{{{word}}}lang")} == {language}
        readers[writer] = []
        for reader, other in binaries.items():
            session = cli(other, "inspect", "--session", copy)
            assert session["text"] == replacement and len(session["pages"]) == 1
            readers[writer].append(reader)
    assert SPELL["sha"](saved) == original
    return readers


def native(old, new, desktop, out):
    czech = out / "desktop-czech-dictionary"
    SPELL["write_dictionary"](
        czech, "cs-CZ", b"SET UTF-8\n", "2\nŠkola\nškola\n".encode()
    )
    windows = {
        "grammar": SPELL["verify_desktop"](
            desktop,
            czech,
            out / "desktop-czech",
            ("Škola škola",),
            ["Škola"],
            "Škola",
            "cs-CZ",
        ),
        "spelling": SPELL["verify_desktop"](
            desktop,
            out / "unicode-split",
            out / "desktop-grapheme",
            ("preq\u0301post",),
            ["preq\u0301 post"],
            "preq\u0301 post",
            "nl",
        ),
    }
    for kind, name, locale in (("grammar", "czech", "cs-CZ"), ("spelling", "grapheme", "nl")):
        row = windows[kind]
        row["roundtripReadersByWriter"] = exchange(
            old, new, out / ("desktop-" + name), locale, row["replacement"]
        )
    return windows


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for item in ("old", "new", "output"):
        p.add_argument(item, type=Path)
    p.add_argument("--desktop", type=Path)
    a = p.parse_args()
    old, new, out = [x.resolve() for x in (a.old, a.new, a.output)]
    out.mkdir(parents=True)
    report = {
        "binaries": [SPELL["sha"](p) for p in (old, new)],
        "nativeGrammar": grammar(old, new, out),
        "graphemeSplits": missing_spaces(old, new, out),
    }
    if a.desktop:
        report["windows"] = native(old, new, a.desktop.resolve(), out)
    (out / "report.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    )
