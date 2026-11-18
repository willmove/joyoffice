"""Compare compound permissions on synthetic dictionaries and the 75 published JoyWord packs."""

import argparse
import hashlib
import itertools
import json
import re
import runpy
import tempfile
from pathlib import Path

morph = runpy.run_path(str(Path(__file__).with_name("check-morphology.py")))
support = morph["support"]


def compare(versions, oracle, aff, dic, words, reference=None, forbidden_endings=()):
    expected = oracle.check(reference or aff, dic, words)
    assert not oracle.diagnostic, oracle.diagnostic
    old, new = [morph["answers"](v, aff, dic, words) for v in versions]
    assert isinstance(old, list) and isinstance(new, list), (aff, old, new)
    rows = list(zip(words, old, new, expected, strict=True))
    protected = [
        w
        for w, old_answer, got, want in rows
        if want
        and not got
        and not old_answer
        and any(w.endswith(end) for end in forbidden_endings)
    ]
    return {
        "retainedForbidden": len(protected),
        "words": len(rows),
        "right": sum(expected),
        "oracleSha256": hashlib.sha256(bytes(expected)).hexdigest(),
        "before": sum(a != c for _, a, b, c in rows),
        "after": sum(b != c for _, a, b, c in rows),
        "newDifferences": [[w, c] for w, a, b, c in rows if a == c and b != c][:8],
        "remainingExamples": [[w, c] for w, a, b, c in rows if b != c][:4],
    }


def dictionary(folder, label, directives, entries):
    aff, dic = folder / (label + ".aff"), folder / (label + ".dic")
    aff.write_text("SET UTF-8\n" + directives + "\n")
    dic.write_text("\n".join([str(len(entries)), *entries]) + "\n")
    return aff, dic


def patterns(folder, versions, oracle):
    results = {}
    letters = "ABCD"
    stems = {"A": "red", "B": "sun", "C": "fog", "D": "dew"}
    words = [
        "".join(pieces)
        for n in range(1, 5)
        for pieces in itertools.product(stems.values(), repeat=n)
    ]
    words.extend(["redxxsun", "rdsun", "redsuns", "suns", "redfoofog"])
    for mode, ids in [
        ("UTF-8", list("ABCD")),
        ("long", ["aA", "bB", "cC", "dD"]),
        ("num", ["11", "22", "33", "44"]),
    ]:
        table = dict(zip(letters, ids))
        tokens = {c: f"({i})" if mode != "UTF-8" else i for c, i in table.items()}
        for rule in [
            "AB",
            "A?BC",
            "A*BC",
            "A?B*C",
            "AB?C?D",
            "A*B*C*D",
            "ABC?",
            "A?A?A*B",
            "A*",
            "ABCDA",
            "ABCD?",
        ]:
            encoded = "".join(tokens.get(ch, ch) for ch in rule)
            label = mode + "-" + str(len(results))
            aff, dic = dictionary(
                folder,
                label,
                f"FLAG {mode}\nCOMPOUNDMIN 2\nCOMPOUNDRULE 1\nCOMPOUNDRULE {encoded}",
                [f"{stems[c]}/{i}" for c, i in table.items()],
            )
            results[label + " " + rule] = compare(versions, oracle, aff, dic, words)
    return results


def permissions(folder, versions, oracle):
    results = {}
    base = (
        "ONLYINCOMPOUND O\nCOMPOUNDPERMITFLAG P\nCOMPOUNDFORBIDFLAG F\n"
        "FORBIDDENWORD H\nNEEDAFFIX N\nKEEPCASE K\nCIRCUMFIX C\nCOMPOUNDMIN 2\n"
    )
    stems = [
        "red",
        "sun",
        "fog",
        "redsun",
        "sunred",
        "sunsun",
        "redfog",
        "redsunen",
        "redsunens",
        "redpresunen",
        "redpresun",
        "redensun",
        "preredsun",
        "redsunred",
        "fogredsun",
        "redredsun",
        "redsunsun",
        "redsunsunen",
        "redmist",
        "redmisten",
        "sunenred",
        "rediced",
        "redsunenen",
    ]
    for grammar in [
        "COMPOUNDRULE 2\nCOMPOUNDRULE AB\nCOMPOUNDRULE A*DB",
        "COMPOUNDBEGIN A\nCOMPOUNDMIDDLE D\nCOMPOUNDEND B",
    ]:
        for edge in ["", "T", "OT", "PT", "FT", "NT", "KT", "BT", "CT"]:
            for flags in ["BS", "S", "BNS", "BOS", "BFS", "BHS"]:
                directives = (
                    base
                    + grammar
                    + "\nSFX S Y 1\nSFX S 0 en"
                    + ("/" + edge if edge else "")
                    + " .\nSFX T Y 1\nSFX T 0 s .\nPFX X Y 1\nPFX X 0 pre/P ."
                )
                label = (
                    ("rules" if "RULE" in grammar else "roles")
                    + "-"
                    + (edge or "bare")
                    + "-"
                    + flags
                )
                aff, dic = dictionary(
                    folder,
                    label,
                    directives,
                    [
                        "red/ASX",
                        "fog/D",
                        "sun/" + flags + "X",
                        "mist/S",
                        "mist/B",
                        "iced/BH",
                    ],
                )
                # The JoyWord dictionary contract also excludes a FORBIDDENWORD
                # root inside a compound, even where Hunspell's rule matcher accepts it.
                protected = ["iced"]
                if "H" in flags:
                    protected += ["sun", "sunen", "sunens"]
                results[label] = compare(
                    versions,
                    oracle,
                    aff,
                    dic,
                    stems,
                    forbidden_endings=protected
                    if grammar.startswith("COMPOUNDRULE")
                    else (),
                )
    for tag, options in [
        ("dup", "CHECKCOMPOUNDDUP"),
        ("case", "CHECKCOMPOUNDCASE"),
        ("triple", "CHECKCOMPOUNDTRIPLE"),
        ("simple", "CHECKCOMPOUNDTRIPLE\nSIMPLIFIEDTRIPLE"),
        ("edge", "CHECKCOMPOUNDPATTERN 1\nCHECKCOMPOUNDPATTERN d/A s/B"),
        ("force", "FORCEUCASE U"),
        ("root", "COMPOUNDWORDMAX 2\nCOMPOUNDROOT R"),
        ("hu", "LANG hu\nCOMPOUNDSYLLABLE 3 aeiuo\nCOMPOUNDWORDMAX 2\nCOMPOUNDROOT R"),
    ]:
        words = [
            "".join(p)
            for n in (2, 3)
            for p in itertools.product(
                ["red", "sun", "fog", "Red", "odd", "drop", "off", "fall"], repeat=n
            )
        ]
        aff, dic = dictionary(
            folder,
            tag,
            base + "COMPOUNDFLAG G\n" + options,
            [
                "red/AG",
                "sun/BG",
                "fog/GU",
                "Red/G",
                "odd/G",
                "drop/G",
                "off/GR",
                "fall/G",
            ],
        )
        results[tag] = compare(versions, oracle, aff, dic, words)
    return results


def locales(folder, versions, oracle):
    results = {}
    words = (
        "istanbul Istanbul İstanbul ISTANBUL İSTANBUL ırmak Irmak IRMAK İrmak ipekler Ipekler İpekler IPEKLER İPEKLER IŞIK İŞİK İĞNE IĞNE İZMİR IZMIR IĞDIR İĞDİR IPHONE İPHONE iPhone".split()
        + ["I\u0307STANBUL"]
    )
    for tag in [
        "en",
        "tr",
        "tr_TR",
        "tr_CY",
        "tr-TR",
        "az",
        "az_AZ",
        "crh",
        "crh_UA",
        "crh_RU",
        "de_DE",
        "hu_HU",
    ]:
        aff, dic = dictionary(
            folder,
            tag,
            "LANG " + tag + "\nKEEPCASE K\nSFX S Y 1\nSFX S 0 ler .",
            [
                "istanbul",
                "ırmak",
                "ipek/S",
                "ışık",
                "iğne",
                "İzmir",
                "Iğdır",
                "iPhone/K",
            ],
        )
        results[tag] = compare(versions, oracle, aff, dic, words)
    return results


def hungarian(folder, versions, oracle):
    results = {}
    for tag in ("hu", "hu_HU", "hu-GB", "en"):
        for extension in ("", "SYLLABLENUM klmc\n"):
            for limit in (2, 3, 4, 5, 6):
                rules = (
                    f"LANG {tag}\nCOMPOUNDMIN 2\nCOMPOUNDFLAG Z\nCOMPOUNDWORDMAX 2\n"
                    f"NOSPLITSUGS\nCOMPOUNDPERMITFLAG O\nCOMPOUNDSYLLABLE {limit} aeiou\n{extension}"
                    "PFX P Y 1\nPFX P 0 anana/O .\nPFX Q Y 1\nPFX Q 0 an/O .\n"
                    "SFX I Y 1\nSFX I 0 nai .\nSFX J Y 1\nSFX J 0 naj .\n"
                    "SFX c Y 1\nSFX c 0 nac .\nSFX S Y 1\nSFX S 0 nat ."
                )
                entries = [
                    "bab/ZPQ",
                    "ded/ZPQ",
                    "fin/ZI",
                    "sun/ZIJ",
                    "tip/Zc",
                    "rig/ZS",
                    "log/ZJ",
                ]
                aff, dic = dictionary(folder, "syllables", rules, entries)
                finals = [
                    "fin",
                    "finami",
                    "finnai",
                    "sun",
                    "sunnai",
                    "sunnaj",
                    "tip",
                    "tipnac",
                    "rig",
                    "rignat",
                    "log",
                    "lognaj",
                ]
                words = [
                    a + b
                    for a, b in itertools.product(
                        (
                            "bab",
                            "babded",
                            "anbabded",
                            "ananababded",
                            "ananabab",
                            "babananaded",
                        ),
                        finals,
                    )
                ]
                words += [
                    "ananababded",
                    "anbabded",
                    "babananaded",
                    "findedbab",
                    "babfinded",
                ]
                results[f"{tag}-{bool(extension)}-{limit}"] = compare(
                    versions, oracle, aff, dic, words
                )
    return results


def pack_words(affix, dictionary):
    charset = re.search(rb"(?mi)^SET[ \t]+([^\s]+)", affix)
    codec = charset[1].decode("ascii") if charset else "utf-8"
    table = affix.decode(codec, errors="replace")
    mode = re.search(r"(?m)^FLAG\s+(\S+)", table)
    mode = mode[1] if mode else "UTF-8"
    tags = set(re.findall(r"(?m)^COMPOUND(?:FLAG|BEGIN|MIDDLE|END)\s+(\S+)", table))
    for rule in re.findall(r"(?m)^COMPOUNDRULE[ \t]+([^\s#]+)", table):
        if not rule.isnumeric():
            if mode in ("num", "long"):
                tags.update(re.findall(r"\(([^)]+)\)", rule))
            else:
                tags.update(ch for ch in rule if ch.isalpha())
    rows = dictionary.decode(codec, errors="replace").splitlines()[1:]
    tagged = []
    ordinary = []
    for part in range(12):
        start = part * len(rows) // 12
        chosen_word = chosen_tag = False
        for raw in rows[start : start + 6000]:
            row = raw.split()
            if not row:
                continue
            word, _, flags = row[0].partition("/")
            if (
                not word.isalpha()
                or not 2 <= len(word) <= 10
                or word.encode(codec, errors="ignore").decode(codec) != word
            ):
                continue
            if not chosen_word:
                ordinary.append(word)
                chosen_word = True
            available = (
                flags.split(",")
                if mode == "num"
                else [flags[i : i + 2] for i in range(0, len(flags), 2)]
                if mode == "long"
                else list(flags)
            )
            if not chosen_tag and tags.intersection(available):
                tagged.append(word)
                chosen_tag = True
            if chosen_word and (chosen_tag or not tags):
                break
    roots = list(dict.fromkeys(tagged + ordinary))
    if not roots:
        return morph["sample"](affix, dictionary)
    words = []
    for i, s in enumerate(roots):
        t = roots[(i + 1) % len(roots)]
        words.extend(
            [
                s,
                t,
                s + t,
                t + s,
                s + s,
                s + t[::-1],
                s + t + roots[(i + 3) % len(roots)],
            ]
        )
    return list(dict.fromkeys(words))


def published(folder, packs, versions, oracle):
    index = packs / "index.json"
    assert support["sha"](index) == support["PUBLISHED_INDEX"]
    results = {}
    for row in json.loads(index.read_text())["packs"]:
        tag = row["locale"]
        archive = packs / (tag + ".tar.zst")
        assert support["sha"](archive) == row["sha256"]
        affix, source = support["published_data"](archive)
        aff, dic = folder / "published.aff", folder / "published.dic"
        aff.write_bytes(affix)
        dic.write_bytes(source)
        words = pack_words(affix, source)
        reference = aff
        oracle.check(aff, dic, [])
        if oracle.diagnostic:
            assert "Failure loading aff file" in oracle.diagnostic, (
                tag,
                oracle.diagnostic,
            )
            corrected, fixes = morph["counted_affix_copy"](affix)
            assert fixes
            reference = folder / "reference.aff"
            reference.write_bytes(corrected)
        results[tag] = compare(versions, oracle, aff, dic, words, reference)
    return results


def rollback(folder, old_cli, new_cli):
    run = runpy.run_path(str(Path(__file__).with_name("check-complex-corpus.py")))[
        "run_cli"
    ]
    saved = folder / "window" / "校对 副本.docx"
    digest = support["sha"](saved)
    text = run(new_cli, "inspect", "--session", saved)["text"]
    assert text == "redsunen\nredsunen\nredsunens\nİSTANBUL\nIRMAK"
    versions = {"baseline": old_cli, "candidate": new_cli}
    pairs = []
    for writer, exe in versions.items():
        copy = folder / (writer + "-resaved.docx")
        run(exe, "convert", saved, copy)
        for reader, binary in versions.items():
            assert run(binary, "inspect", "--session", copy)["text"] == text
            pairs.append([writer, reader])
    assert support["sha"](saved) == digest
    return {
        "savedSha256": digest,
        "pairs": pairs,
        "cliSha256": {k: support["sha"](b) for k, b in versions.items()},
    }


def review(old_cli, new_cli, desktop, output):
    output.mkdir(parents=True)
    folder = output / "dictionary"
    rules = (
        "SET UTF-8\nLANG tr_TR\nCOMPOUNDMIN 2\nCOMPOUNDRULE 1\nCOMPOUNDRULE AB\n"
        "TRY redsunaylk\nSFX S Y 1\nSFX S 0 en/T .\nSFX T Y 1\nSFX T 0 s .\n"
    )
    support["write_dictionary"](
        folder,
        "tr",
        rules.encode(),
        "5\nred/A\nsun/BS\nistanbul\nırmak\nwalk\n".encode(),
    )
    words = ["redsunenn", "redsunen", "redsunens", "İSTANBUL", "IRMAK"]
    old, new = [
        support["results"](binary, folder, "tr", words) for binary in (old_cli, new_cli)
    ]
    assert [row["text"] for row in new] == [words[0]], new
    assert {r["text"] for r in old} >= {words[0], words[1], words[2]}, old
    ordered = new[0]["suggestions"]
    assert words[1] in ordered, (old, new)
    native = support["verify_desktop"](
        desktop, folder, output / "window", words, ordered, words[1], locale="tr"
    )
    return {
        "oldIssues": old,
        "newIssues": new,
        "native": native,
        "rollback": rollback(output, old_cli, new_cli),
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["baseline", "candidate", "reference_library", "output"]:
        parser.add_argument(name, type=Path)
    parser.add_argument("--packs", type=Path)
    parser.add_argument(
        "--review",
        nargs=4,
        type=Path,
        metavar=("OLD_CLI", "NEW_CLI", "DESKTOP", "DIRECTORY"),
    )
    args = parser.parse_args()
    oracle = morph["Hunspell"](args.reference_library)
    versions = [args.baseline.resolve(), args.candidate.resolve()]
    with tempfile.TemporaryDirectory() as temp:
        folder = Path(temp)
        data = {
            name: fn(folder, versions, oracle)
            for name, fn in [
                ("patterns", patterns),
                ("permissions", permissions),
                ("locales", locales),
                ("hungarian", hungarian),
            ]
        }
        if args.packs:
            data["published"] = published(folder, args.packs, versions, oracle)
    if args.review:
        native = review(*(x.resolve() for x in args.review))
        (args.review[-1] / "report.json").write_text(
            json.dumps(native, ensure_ascii=False, indent=2) + "\n"
        )
    args.output.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    for group, rows in data.items():
        print(
            group,
            len(rows),
            *(
                sum(row[k] for row in rows.values())
                for k in ["words", "right", "before", "after"]
            ),
            flush=True,
        )
    if any(
        row["newDifferences"] or row["after"] != row["retainedForbidden"]
        for rows in data.values()
        for row in rows.values()
    ):
        raise SystemExit("new reference differences")
