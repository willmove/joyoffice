"""Check localized lexical corrections against expected byte spans and two Word readers."""

import argparse
import hashlib
import json
from pathlib import Path
import runpy
from xml.sax.saxutils import escape

BOUNDARIES = runpy.run_path(
    str(Path(__file__).with_name("check-language-boundaries.py"))
)
SPELL = BOUNDARIES["SPELL"]
REGIONS = [
    "en-US",
    "en-GB",
    "en-CA",
    "en-AU",
    "es-ES",
    "es-MX",
    "es-419",
    "es-UY",
    "it-IT",
    "ru-RU",
    "pl-PL",
    "tr-TR",
]


def examples(locale):
    base = locale[:2]
    if base == "en":
        for word, expected in [
            ("apple", "an"),
            ("eagle", "an"),
            ("hour", "an"),
            ("honest", "an"),
            ("heir", "an"),
            ("university", "a"),
            ("uniform", "a"),
            ("unidentified", "an"),
            ("one", "a"),
            ("European", "a"),
            ("eulogy", "a"),
            ("MRI", "an"),
            ("FBI", "an"),
            ("UFO", "a"),
            ("USB", "a"),
            ("dog", "a"),
            ("historic", None),
            ("herb", None),
            ("hors", None),
            ("urologist", "a"),
            ("SQL", None),
            ("LED", None),
        ]:
            for source in ("a", "an"):
                fix = (
                    []
                    if source == expected or expected is None
                    else [(source, expected)]
                )
                yield f"We chose {source} {word} today.", fix
    elif base == "es":
        for head, span in [
            ("Vienes", "Vienes"),
            ("Ahora: vienes", "vienes"),
            ("Son las 10:30. Vienes", "Vienes"),
            ("¿Está bien? Pedro viene", "Pedro viene"),
            ("En https://ab.es/¿? Vienes", "Vienes"),
            ("Dijo «Vienes", "Vienes"),
            ("Un precio de 3.5; cuesta más", "cuesta más"),
        ]:
            for mark, opener in [("?", "¿"), ("!", "¡")]:
                yield head + mark, [(span + mark, opener + span + mark)]
        for sentence in [
            "¿Vienes?",
            "¡¡Qué bien!!",
            "¿Vendrás ¡hoy!?",
            "¿¡Vendrás!?",
            "Tal vez (?) sí.",
            "Visita https://ab.es/?a=1",
            "Dijo (?) y siguió.",
        ]:
            yield sentence, []
    elif base == "it":
        for sentence, fixes in [
            ("Vorrei un pò.", [("pò", "po'")]),
            ("Vorrei un po.", [("po", "po'")]),
            ("Vorrei un po’.", []),
            ("Il Po è lungo.", []),
            ("Qual’è?", [("Qual’è", "Qual è")]),
            ("QUAL'ERA?", [("QUAL'ERA", "QUAL ERA")]),
            ("E’ arrivata.", [("E’", "È")]),
            ("La lettera ‘E’ è qui.", []),
            ("Un amica.", [("Un amica", "Un'amica")]),
            ("Un'altra.", []),
            ("È un'anno.", [("un'anno", "un anno")]),
            ("Un'automobile.", []),
            ("Un Anna.", []),
            ("Un acrobata.", []),
        ]:
            yield sentence, fixes
    elif base == "ru":
        for word in ["что", "чтобы", "когда", "поскольку"]:
            yield f"Знаю {word} придёт.", [(" " + word, ", " + word)]
            yield f"Знаю, {word} придёт.", []
        for s, fixes in [
            ("Ушёл потому что устал.", [(" потому", ", потому")]),
            ("Ушёл потому, что устал.", []),
            ("Сделай хотя бы раз.", []),
            ("Редко когда удаётся.", []),
            ("Не знаю когда.", []),
            ("Сказал\u00a0что знает.", [("\u00a0что", ", что")]),
        ]:
            yield s, fixes
    elif base == "pl":
        for word in ["że", "żeby", "który"]:
            yield f"Wiem {word} czytasz.", [(" " + word, ", " + word)]
            yield f"Wiem, {word} czytasz.", []
        for s, fixes in [
            ("Dom w którym mieszkam.", []),
            ("Z szkoły.", [("Z", "Ze")]),
            ("Idę z szafy.", []),
            ("Idę w wtorek.", [("w", "we")]),
            ("Idę we wtorek.", []),
            ("Idę z\u00a0mną.", [("z", "ze")]),
        ]:
            yield s, fixes
    else:
        for word in ["ve", "veya", "yahut"]:
            yield f"Ali, {word} Veli.", [(",", "")]
            yield f"Ali {word}, Veli.", [(",", "")]
            yield f"Ali {word} Veli.", []
            yield f"Ali,\n{word} Veli.", []
            yield f"«Ali,» {word} Veli.", []
            yield f"Ali {word} «,Veli».", []


def target(locale, row):
    msg = row["message"]
    return any(
        msg.startswith(prefix)
        for prefix in {
            "en": ["Use "],
            "es": ["Falta el signo"],
            "pl": ["Brak przecinka", "Przed tym"],
            "ru": ["Возможно, пропущена"],
            "tr": ["'"],
            "it": ["Si scrive", "«Qual", "«È»", "Davanti a"],
        }[locale[:2]]
    )


def compare(old, new, path, region):
    path.mkdir()
    cases = list(examples(region))
    items = [row[0] for row in cases]
    if any("\n" in t for t in items):
        make = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))
        body = []
        for t in items:
            inline = "<w:br/>".join(
                f'<w:t xml:space="preserve">{escape(piece)}</w:t>'
                for piece in t.split("\n")
            )
            body.append(
                f'<w:p><w:r><w:rPr><w:lang w:val="{region}"/></w:rPr>{inline}</w:r></w:p>'
            )
        make["package"](path / "sample.docx", "".join(body) + "<w:sectPr/>")
    outputs = [
        SPELL["results"](cli, path, region, items, "grammar") for cli in (old, new)
    ]
    baseline, current = outputs
    for issue in current:
        index = issue["start"]["path"][0]
        assert (
            items[index].encode()[issue["start"]["off"] : issue["end"]["off"]].decode()
            == issue["text"]
        ), issue
    for i, (sentence, expected) in enumerate(cases):
        issues = [
            r for r in current if r["start"]["path"][0] == i and target(region, r)
        ]
        actual = [(r["text"], r["suggestions"]) for r in issues]
        assert actual == [(frag, [fix]) for frag, fix in expected], (
            region,
            sentence,
            expected,
            actual,
        )
        for r in issues:
            assert region.split("-")[0] == r["lang"].split("-")[0], r
    assert [r for r in baseline if not target(region, r)] == [
        r for r in current if not target(region, r)
    ]
    return {
        "cases": len(cases),
        "sameIssues": sum(r in baseline for r in current),
        "replacedOrRemoved": sum(r not in current for r in baseline),
        "newOrCorrected": sum(r not in baseline for r in current),
        "orderedGrammarSha256": hashlib.sha256(
            json.dumps(current, ensure_ascii=False, sort_keys=True).encode()
        ).hexdigest(),
    }


def windows(old, new, desktop, out):
    result = {}
    for region, phrase, fix, words in [
        ("es-ES", "Vienes?", "¿Vienes?", "Vienes"),
        ("it-IT", "Un amica", "Un'amica", "Un\namica\nUn'amica"),
    ]:
        path = out / ("window-" + region)
        dictionary = out / ("dictionary-" + region)
        SPELL["write_dictionary"](
            dictionary,
            region,
            b"SET UTF-8\n",
            (f"{len(words.splitlines())}\n{words}\n").encode(),
        )
        row = SPELL["verify_desktop"](
            desktop, dictionary, path, (phrase,), [fix], fix, region
        )
        row["readersByWriter"] = BOUNDARIES["exchange"](old, new, path, region, fix)
        result[region] = row
    return result


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    for key in ["baseline", "candidate", "out"]:
        ap.add_argument(key, type=Path)
    ap.add_argument("--desktop", type=Path)
    args = ap.parse_args()
    old, new, out = (p.resolve() for p in (args.baseline, args.candidate, args.out))
    out.mkdir(parents=True)
    report = {
        "binaries": [SPELL["sha"](b) for b in (old, new)],
        "profiles": {r: compare(old, new, out / r, r) for r in REGIONS},
    }
    if args.desktop:
        report["desktop"] = windows(old, new, args.desktop.resolve(), out)
    (out / "report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    )
