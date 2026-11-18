"""Verify Word's visible suggestions and the effect of the forward generator on released language packs."""

import argparse
import json
import runpy
from pathlib import Path

morph = runpy.run_path(str(Path(__file__).with_name("check-morphology.py")))
support = morph["support"]


def compare(binaries, packs, oracle, out):
    index = packs / "index.json"
    assert support["sha"](index) == support["PUBLISHED_INDEX"]
    result = {}
    for entry in json.loads(index.read_text())["packs"]:
        language = entry["locale"]
        archive = packs / (language + ".tar.zst")
        assert support["sha"](archive) == entry["sha256"]
        affix, dictionary = support["published_data"](archive)
        folder = out / language
        support["write_dictionary"](folder, language, affix, dictionary)
        probes = support["edit_probes"](affix, dictionary)
        before, after = [
            support["results"](binary, folder, language, probes) for binary in binaries
        ]
        changes = []
        added = []
        for old, new in zip(before, after, strict=True):
            assert {k: v for k, v in old.items() if k != "suggestions"} == {
                k: v for k, v in new.items() if k != "suggestions"
            }
            left, right = old["suggestions"], new["suggestions"]
            if left == right:
                continue
            kept = [w for w in left if w in right]
            assert kept == [w for w in right if w in left], (language, left, right)
            added.extend(w for w in right if w not in left)
            changes.append({"word": old["text"], "old": left, "new": right})
        if added:
            aff = folder / (language + ".aff")
            dic = folder / (language + ".dic")
            answers = oracle.check(aff, dic, added)
            if oracle.diagnostic:
                assert "Failure loading aff file" in oracle.diagnostic
                corrected, fixes = morph["counted_affix_copy"](affix)
                assert fixes
                aff = folder / "reference.aff"
                aff.write_bytes(corrected)
                answers = oracle.check(aff, dic, added)
                assert not oracle.diagnostic
            assert all(answers), (
                language,
                [w for w, yes in zip(added, answers) if not yes],
            )
        result[language] = {
            "inputs": len(probes),
            "issues": len(after),
            "changes": changes,
        }
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in (
        "baseline",
        "candidate",
        "desktop",
        "packs",
        "reference_library",
        "output",
    ):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    binaries = [args.baseline.resolve(), args.candidate.resolve()]
    out = args.output.resolve()
    out.mkdir(parents=True)
    folder = out / "derived-dictionary"
    aff = b"SET UTF-8\nCIRCUMFIX c\nPFX P Y 1\nPFX P 0 wo/c .\nSFX A Y 1\nSFX A 0 able/Bc .\nSFX B Y 1\nSFX B 0 s .\n"
    support["write_dictionary"](folder, "nl", aff, b"1\nwork/AP\n")
    typo, word = "waworiables", "woworkables"
    probes = [typo, word, "work"]
    old, new = [support["results"](binary, folder, "nl", probes) for binary in binaries]
    assert [row["text"] for row in old] == [row["text"] for row in new] == [typo]
    assert word not in old[0]["suggestions"] and new[0]["suggestions"] == [word], (
        old,
        new,
    )
    native = support["verify_desktop"](
        args.desktop.resolve(), folder, out / "window", probes, [word], word
    )
    result = {
        "wordCliSha256": [support["sha"](b) for b in binaries],
        "old": old,
        "new": new,
        "native": native,
    }
    result["languages"] = compare(
        binaries,
        args.packs.resolve(),
        morph["Hunspell"](args.reference_library.resolve()),
        out,
    )
    (out / "report.json").write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n"
    )


if __name__ == "__main__":
    main()
