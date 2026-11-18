"""Validate forward affix generation against independent Hunspell acceptance and prior output."""

import argparse
import hashlib
import itertools
import json
import re
import runpy
import subprocess
import tempfile
from pathlib import Path

morph = runpy.run_path(str(Path(__file__).with_name("check-morphology.py")))
support = morph["support"]


def expand(binary, aff, dic, stems):
    p = subprocess.run(
        [binary, "forms", aff, dic],
        input="\n".join(stems) + "\n",
        text=True,
        capture_output=True,
        check=True,
        timeout=180,
    )
    rows = [line.split("\t") for line in p.stdout.splitlines()]
    assert [r[0] for r in rows] == stems
    return {r[0]: set(w for w in r[1:] if w) for r in rows}


def matrix(baseline, candidate, oracle, folder):
    aff, dic = folder / "case.aff", folder / "case.dic"
    words = ["work", "workable", "workables", "nework", "neworkable", "neworkables"]
    total = old_errors = new_errors = 0
    failed = []
    reference = hashlib.sha256()
    for pc, ic, oc in itertools.product((False, True), repeat=3):
        for p, i, o in itertools.product("NY", repeat=3):
            for source in (
                "stem",
                "inner",
                "outer",
                "stem+inner",
                "stem+outer",
                "inner+outer",
                "stem+inner+outer",
                "split",
                "none",
            ):
                for needs in ("", "P", "I", "PI", "O", "PO", "IO", "PIO"):

                    def flags(s):
                        return "/" + s if s else ""

                    aff.write_text(
                        "SET UTF-8\nCIRCUMFIX c\nNEEDAFFIX N\n"
                        + f"PFX P {p} 1\nPFX P 0 ne{flags('c' * pc + 'N' * ('P' in needs))} .\n"
                        + f"SFX A {i} 1\nSFX A 0 able{flags('B' + 'c' * ic + 'P' * ('inner' in source) + 'N' * ('I' in needs))} .\n"
                        + f"SFX B {o} 1\nSFX B 0 s{flags('c' * oc + 'P' * ('outer' in source) + 'N' * ('O' in needs))} .\n"
                    )
                    dic.write_text(
                        "2\nwork/A\nwork/P\n"
                        if source == "split"
                        else "1\nwork/A" + ("P" if "stem" in source else "") + "\n"
                    )
                    expected = {
                        w for w, yes in zip(words, oracle.check(aff, dic, words)) if yes
                    }
                    old = expand(baseline, aff, dic, ["work"])["work"]
                    new = expand(candidate, aff, dic, ["work"])["work"]
                    old_errors += len(old ^ expected)
                    new_errors += len(new ^ expected)
                    total += 1
                    reference.update(bytes(w in expected for w in words))
                    if new != expected and len(failed) < 12:
                        failed.append(
                            [
                                pc,
                                ic,
                                oc,
                                p,
                                i,
                                o,
                                source,
                                needs,
                                sorted(expected - new),
                                sorted(new - expected),
                            ]
                        )
    return dict(
        grammars=total,
        decisions=total * len(words),
        baselineDifferences=old_errors,
        candidateDifferences=new_errors,
        examples=failed,
        referenceSha256=reference.hexdigest(),
    )


def sampled(affix, dictionary):
    codec = re.search(rb"(?mi)^SET[ \t]+([^\s]+)", affix)
    charset = codec[1].decode("ascii") if codec else "utf-8"
    rows = dictionary.decode(charset, errors="replace").splitlines()[1:]
    result = []
    for index in range(12):
        start = (2 * index + 1) * len(rows) // 24
        for raw in rows[start : start + 1800]:
            stem, sep, flags = raw.partition("/")
            if (
                sep
                and flags.strip()
                and stem.isalpha()
                and 2 <= len(stem) <= 18
                and stem not in result
            ):
                result.append(stem)
                break
    if not result:
        result = morph["sample"](affix, dictionary)[:12]
    return result


def published(binaries, oracle, folder, packs):
    index = packs / "index.json"
    assert support["sha"](index) == support["PUBLISHED_INDEX"]
    reports = {}
    for row in json.loads(index.read_text())["packs"]:
        locale = row["locale"]
        archive = packs / (locale + ".tar.zst")
        assert support["sha"](archive) == row["sha256"]
        affix, dictbytes = support["published_data"](archive)
        stems = sampled(affix, dictbytes)
        assert stems
        aff, dic = folder / "release.aff", folder / "release.dic"
        aff.write_bytes(affix)
        dic.write_bytes(dictbytes)
        old, new = (expand(binary, aff, dic, stems) for binary in binaries)
        union = sorted(set().union(*old.values(), *new.values()))
        ref = aff
        references = oracle.check(ref, dic, union)
        if oracle.diagnostic:
            assert "Failure loading aff file" in oracle.diagnostic
            corrected, counts = morph["counted_affix_copy"](affix)
            assert counts
            ref = folder / "reference.aff"
            ref.write_bytes(corrected)
            references = oracle.check(ref, dic, union)
            assert not oracle.diagnostic
        expected = {w for w, correct in zip(union, references) if correct}
        report = {
            "stems": len(stems),
            "formsChecked": len(union),
            "referenceAccepted": len(expected),
            "baselineRejected": sum(len(values - expected) for values in old.values()),
            "candidateRejected": sum(len(values - expected) for values in new.values()),
            "newAccepted": sum(len((new[t] - old[t]) & expected) for t in stems),
            "removedAccepted": sum(len((old[t] - new[t]) & expected) for t in stems),
            "candidateExamples": [
                [t, w] for t in stems for w in sorted(new[t] - expected)
            ][:4],
            "removedExamples": [
                [t, w] for t in stems for w in sorted((old[t] - new[t]) & expected)
            ][:4],
        }
        removed = 0
        if report["removedAccepted"]:
            codec = re.search(rb"(?mi)^SET[ \t]+([^\s]+)", affix)
            charset = codec[1].decode("ascii") if codec else "utf-8"
            source = dictbytes.decode(charset).splitlines()[1:]
            for stem in stems:
                missing = sorted((old[stem] - new[stem]) & expected)
                if not missing:
                    continue
                entries = [
                    r for r in source if r.partition("/")[0].split("\t")[0] == stem
                ]
                assert entries, (locale, stem)
                reduced = folder / "same-root.dic"
                reduced.write_bytes(
                    ("\n".join([str(len(entries)), *entries]) + "\n").encode(charset)
                )
                derived = oracle.check(ref, reduced, missing)
                assert not oracle.diagnostic, (locale, oracle.diagnostic)
                removed += sum(derived)
        report["removedDerivableFromSameStem"] = removed
        reports[locale] = report
    return reports


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    for k in ("baseline", "candidate", "reference_library", "output"):
        p.add_argument(k, type=Path)
    p.add_argument("--packs", type=Path)
    args = p.parse_args()
    oracle = morph["Hunspell"](args.reference_library)
    with tempfile.TemporaryDirectory() as temporary:
        folder = Path(temporary)
        data = {
            "referenceSha256": support["sha"](args.reference_library),
            "programs": [support["sha"](b) for b in (args.baseline, args.candidate)],
            "grammar": matrix(args.baseline, args.candidate, oracle, folder),
        }
        if args.packs:
            data["published"] = published(
                (args.baseline, args.candidate), oracle, folder, args.packs
            )
    args.output.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    assert data["grammar"]["candidateDifferences"] == 0, data["grammar"]
    if "published" in data:
        assert all(
            r["candidateRejected"] == 0 and r["removedDerivableFromSameStem"] == 0
            for r in data["published"].values()
        ), {
            name: r
            for name, r in data["published"].items()
            if r["candidateRejected"] or r["removedDerivableFromSameStem"]
        }
