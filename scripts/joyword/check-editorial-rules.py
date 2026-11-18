"""Verify localized grammar messages, source ranges, sentence starts and native DOCX corrections."""

import argparse
import hashlib
import json
from pathlib import Path
import runpy

BOUNDARIES = runpy.run_path(
    str(Path(__file__).with_name("check-language-boundaries.py"))
)
SPELL = BOUNDARIES["SPELL"]
LOCALES = [*BOUNDARIES["LANGS"], "pt-PT", "en-US", "en-GB", "en-AU", "en-CA"]


def examples(locale):
    short = locale[:2]
    capital = short in {"en", "it", "pl", "hr", "pt", "ru", "tr"}
    doubled = short in {"it", "pl", "hr", "pt", "ru"}
    quoted = short in {"pl", "ru"}
    return [
        ("Zebra , Beta", " ,", [","]),
        ("Zebra   Beta", "   ", [" "]),
        ("Zebra .5", " .", None),
        ("Zebra ...", "..", None),
        ("Zebra....", "..", None),
        ("Zebra ;)", " ;", None),
        ("Zebra 3 : 1", " :", None),
        ("Zebra 3 : Beta", " :", None if short == "en" else [":"]),
        ("Zebra !»", " !", ["!"] if short in {"es", "ru", "pl", "tr"} else None),
        ("¿ Zebra?", "¿ ", ["¿"] if short == "es" else None),
        ("¡  Zebra!", "¡ ", ["¡"] if short == "es" else None),
        ("Zebra,, Beta", ",,", [","] if doubled else None),
        (
            "Zebra ,,Beta",
            ",,",
            (["„"] if short == "pl" else ["«"])
            if quoted
            else [","]
            if doubled
            else None,
        ),
        ("Zebra..", "..", ["."] if short == "it" else [".", "…"] if doubled else None),
        ("Zebra. alfa", "alfa", ["Alfa"] if capital else None),
        ("Zebra? alfa", "alfa", ["Alfa"] if capital else None),
        ("Zebra... alfa", "alfa", None),
        ("Zebra.alfa", "alfa", None),
        ("«Vai?» alfa", "alfa", ["Alfa"] if short in {"pl", "hr", "ru"} else None),
        ("Zebra. —alfa", "alfa", ["Alfa"] if short in {"it", "hr", "en"} else None),
        ("3. alfa", "alfa", ["Alfa"] if short == "en" else None),
        ("A. alfa", "alfa", ["Alfa"] if short == "en" else None),
        (
            "Zebra. izmir",
            "izmir",
            ["İzmir" if short == "tr" else "Izmir"] if capital else None,
        ),
    ]


def cases(locale):
    explicit = examples(locale)
    # Exercise runs, adjacent quotes, punctuation in URLs and numerical contexts.
    contexts = [
        (left + mark * count + right)
        for left in ("Zebra ", "3 ", "Ёж")
        for mark in ",.;:?!"
        for count in (1, 2, 3, 4, 5, 6)
        for right in (" Beta", "»", "2", "-)", "D", "")
    ]
    return explicit, list(dict.fromkeys([row[0] for row in explicit] + contexts))


def removed(locale, paragraph, issue):
    start = issue["start"]["off"]
    source = paragraph.encode()
    if (
        locale.startswith("en")
        and issue["message"] == "Remove the space before the punctuation"
    ):
        tail = source[start + 2 :].decode()
        return (issue["text"] == " ." and tail[:1] in "0123456789" and bool(tail)) or (
            issue["text"] == " ;"
            and (
                tail[:1] in (")", "(")
                or tail.startswith(("-)", "-("))
                or (
                    tail.startswith(("D", "P"))
                    and (len(tail) == 1 or tail[1].isspace())
                )
            )
        )
    if (
        locale == "it-IT"
        and issue["message"] == "Punteggiatura doppia"
        and issue["text"] == ".."
    ):
        return start > 0 and source[start - 1 : start] == b"."
    return False


def compare(old, new, out, locale):
    explicit, paragraphs = cases(locale)
    expected = {}
    for sentence, fragment, suggestions in explicit:
        expected.setdefault(sentence, []).append((fragment, suggestions))
    summary = {
        "paragraphs": len(paragraphs),
        "independentExpectations": len(explicit),
        "matchingGrammarIssues": 0,
        "correctedFalsePositives": 0,
    }
    digest = hashlib.sha256()
    for offset in range(0, len(paragraphs), 72):
        batch = paragraphs[offset : offset + 72]
        folder = out / f"{locale}-{offset}"
        folder.mkdir()
        before, after = [
            SPELL["results"](cli, folder, locale, batch, "grammar")
            for cli in (old, new)
        ]
        kept = [
            row
            for row in before
            if not removed(locale, batch[row["start"]["path"][0]], row)
        ]
        assert kept == after, (
            locale,
            offset,
            [(batch[row["start"]["path"][0]], row) for row in kept if row not in after][
                :5
            ],
            [(batch[row["start"]["path"][0]], row) for row in after if row not in kept][
                :5
            ],
        )
        summary["matchingGrammarIssues"] += len(after)
        summary["correctedFalsePositives"] += len(before) - len(after)
        digest.update(json.dumps(after, sort_keys=True, ensure_ascii=False).encode())
        for index, sentence in enumerate(batch):
            findings = [row for row in after if row["start"]["path"][0] == index]
            for row in findings:
                assert (
                    sentence.encode()[row["start"]["off"] : row["end"]["off"]].decode()
                    == row["text"]
                )
            for fragment, suggestions in expected.get(sentence, []):
                candidates = [row for row in findings if row["text"] == fragment]
                assert [row["suggestions"] for row in candidates] == (
                    [] if suggestions is None else [suggestions]
                ), (locale, sentence, fragment, findings)
    summary["orderedIssuesSha256"] = digest.hexdigest()
    return summary


def desktop(old, new, program, out):
    runs = {}
    for language, word, fixes in [
        ("pl-PL", "..", [".", "…"]),
        ("tr-TR", "izmir", ["İzmir"]),
    ]:
        folder = out / ("window-" + language)
        dictionary = out / ("dictionary-" + language)
        SPELL["write_dictionary"](
            dictionary, language, b"SET UTF-8\n", "2\nizmir\nİzmir\n".encode()
        )
        row = SPELL["verify_desktop"](
            program, dictionary, folder, (word,), fixes, fixes[0], language
        )
        row["readersByWriter"] = BOUNDARIES["exchange"](
            old, new, folder, language, fixes[0]
        )
        runs[language] = row
    return runs


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ("old", "new", "out"):
        parser.add_argument(key, type=Path)
    parser.add_argument("--desktop", type=Path)
    parser.add_argument("--locales", nargs="+", choices=LOCALES, default=LOCALES)
    a = parser.parse_args()
    old, new, out = [p.resolve() for p in (a.old, a.new, a.out)]
    out.mkdir(parents=True)
    report = {
        "binaries": [SPELL["sha"](cli) for cli in (old, new)],
        "profiles": {lang: compare(old, new, out, lang) for lang in a.locales},
    }
    if a.desktop:
        report["desktop"] = desktop(old, new, a.desktop.resolve(), out)
    (out / "report.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    )
