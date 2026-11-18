"""Check Hunspell flag syntax and compact dictionary interchange through both Word generations."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
from tempfile import TemporaryDirectory

MORPH = runpy.run_path(str(Path(__file__).with_name("check-morphology.py")))
SPELL = MORPH["support"]


def compact_answers(executable, compact, words):
    output = subprocess.check_output(
        [executable, "check", compact], input="\n".join(words) + "\n", text=True, timeout=180
    )
    rows = [line.split("\t") for line in output.splitlines()]
    assert [v[0] for v in rows] == words and all(v[1] in {"0", "1"} for v in rows)
    return [v[1] == "1" for v in rows]


def compare(folder, programs, reference, aff, dic, words):
    aff_path, dic_path = folder / "sample.aff", folder / "sample.dic"
    aff_path.write_bytes(aff)
    dic_path.write_bytes(dic)
    source = [MORPH["answers"](reader, aff_path, dic_path, words) for reader in programs[0]]
    assert source[0] == source[1], source
    if isinstance(source[0], dict):
        for writer in programs[1]:
            result = subprocess.run([writer, "pack", aff_path, dic_path, folder / "rejected.affix"],
                                    capture_output=True, text=True, timeout=180)
            assert result.returncode and source[0]["readError"] == result.stderr.strip().splitlines()[-1]
        return {**source[0], "words": len(words)}
    expected = reference.check(aff_path, dic_path, words)
    diagnostics = {}
    if reference.diagnostic:
        assert "Failure loading aff file" in reference.diagnostic, reference.diagnostic
        corrected, counts = MORPH["counted_affix_copy"](aff)
        assert counts, reference.diagnostic
        corrected_path = folder / "reference.aff"
        corrected_path.write_bytes(corrected)
        diagnostics = {"originalReferenceDiagnostic": reference.diagnostic, "referenceCopyHeaderCounts": counts,
                       "originalReferenceDifferences": sum(x != y for x, y in zip(expected, source[0], strict=True)),
                       "countedAffixSha256": SPELL["sha"](corrected_path)}
        expected = reference.check(corrected_path, dic_path, words)
        assert not reference.diagnostic and aff_path.read_bytes() == aff
    hashes = []
    for label, writer in enumerate(programs[1]):
        compact = folder / f"{label}.affix"
        subprocess.run([writer, "pack", aff_path, dic_path, compact], capture_output=True, check=True, timeout=180)
        hashes.append(SPELL["sha"](compact))
        for reader in programs[1]:
            assert compact_answers(reader, compact, words) == source[0]
    assert hashes[0] == hashes[1], hashes
    return {
        **diagnostics, "words": len(words), "accepted": sum(source[0]),
        "referenceDifferences": sum(a != b for a, b in zip(expected, source[0], strict=True)),
        "referenceDiagnostic": bool(reference.diagnostic), "answersSha256": hashlib.sha256(bytes(source[0])).hexdigest(),
        "compactSha256": hashes, "bothReadersOnBothWriters": True,
    }


def grammars(folder, versions, oracle):
    stems = ["ad", "cat", "work", "café", "echo", "king", "木刻", "鉛筆", "水木", "acacia"]
    words = [candidate for stem in stems for candidate in (stem, "pre" + stem, stem + "s", "pre" + stem + "s")]
    result = {}
    for mode, pre, suffix, both in [("UTF-8", "P", "S", "PS"), ("long", "Px", "Sy", "PxSy"), ("num", "14", "65000", "14,65000")]:
        dic = (str(len(stems)) + "\n" + "\n".join(stem + "/" + both for stem in stems) + "\n").encode()
        for prefix_condition, suffix_condition in [
            (".", "."), ("[ack]", "[dkt]"), ("[^ae]", "[^ae]"), ("[c木水]", "[刻木é]"),
            (".a", "r."), ("[ac][di]", "[ae][ték]"), ("c", "é"), ("[^木]a", "[^t]k"),
        ]:
            aff = (f"SET UTF-8\nFLAG {mode}\nPFX {pre} Y 1\nPFX {pre} 0 pre {prefix_condition}\n"
                   f"SFX {suffix} Y 1\nSFX {suffix} 0 s {suffix_condition}\n").encode()
            row = compare(folder, versions, oracle, aff, dic, words)
            assert not row["referenceDiagnostic"] and row["referenceDifferences"] == 0, (mode, prefix_condition, suffix_condition, row)
            result[f"{mode} {prefix_condition} {suffix_condition}"] = row
    return result


def review(old, new, desktop, out):
    dictionary = out / "dictionary"
    aff = b"SET UTF-8\nFLAG num\nTRY workse\nSFX 65535 N 1\nSFX 65535 0 s [ck]\n"
    SPELL["write_dictionary"](dictionary, "nl", aff, b"1\nwork/65535\n")
    before, current = [SPELL["results"](cli, dictionary, "nl", ["workss", "works"]) for cli in (old, new)]
    assert current == before and [row["text"] for row in current] == ["workss"], current
    assert current[0]["suggestions"][0] == "works", current
    window = out / "window"
    native = SPELL["verify_desktop"](desktop, dictionary, window, ["workss"], current[0]["suggestions"], "works", "nl")
    shared = runpy.run_path(str(Path(__file__).with_name("check-language-boundaries.py")))
    native["readersByWriter"] = shared["exchange"](old, new, window, "nl", "works")
    return native


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("tools", type=Path, help="directory containing check-{baseline,candidate} and compact-{baseline,candidate}")
    ap.add_argument("reference", type=Path, help="Hunspell shared library")
    ap.add_argument("packs", type=Path)
    ap.add_argument("output", type=Path)
    ap.add_argument("--review", nargs=3, type=Path, metavar=("OLD_CLI", "NEW_CLI", "DESKTOP"))
    a = ap.parse_args()
    tools = [[(a.tools / (kind + "-" + tag)).resolve() for tag in ("baseline", "candidate")] for kind in ("check", "compact")]
    oracle = MORPH["Hunspell"](a.reference.resolve())
    out = a.output.resolve()
    out.mkdir(parents=True)
    index = a.packs / "index.json"
    assert SPELL["sha"](index) == SPELL["PUBLISHED_INDEX"]
    data = {"toolSha256": [[SPELL["sha"](v) for v in pair] for pair in tools],
            "hunspellSha256": SPELL["sha"](a.reference.resolve()), "releaseIndexSha256": SPELL["sha"](index), "packs": {}}
    with TemporaryDirectory() as temp:
        folder = Path(temp)
        data["grammars"] = grammars(folder, tools, oracle)
        for entry in json.loads(index.read_text())["packs"]:
            tag = entry["locale"]
            archive = a.packs / (tag + ".tar.zst")
            assert archive.stat().st_size == entry["size"] and SPELL["sha"](archive) == entry["sha256"]
            aff, dic = SPELL["published_data"](archive)
            words = SPELL["sample_words"](aff, dic)
            data["packs"][tag] = compare(folder, tools, oracle, aff, dic, words)
    if a.review:
        data["desktop"] = review(*(v.resolve() for v in a.review), out)
    (out / "report.json").write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
