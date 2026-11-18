"""Compare JoyWord affix recognition with a standalone Hunspell library and an earlier JoyWord build."""
import argparse
import ctypes
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import runpy
import subprocess
import tempfile

support = runpy.run_path(str(Path(__file__).with_name("check-spelling-order.py")))


class Hunspell:
    def __init__(self, file):
        self.lib = ctypes.CDLL(str(file))
        self.lib.Hunspell_create.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
        self.lib.Hunspell_create.restype = ctypes.c_void_p
        self.lib.Hunspell_destroy.argtypes = [ctypes.c_void_p]
        self.lib.Hunspell_destroy.restype = None
        self.lib.Hunspell_spell.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.Hunspell_spell.restype = ctypes.c_int
        self.lib.Hunspell_get_dic_encoding.argtypes = [ctypes.c_void_p]
        self.lib.Hunspell_get_dic_encoding.restype = ctypes.c_char_p

    def check(self, aff, dic, words):
        # The official C API can return a handle after rejecting an affix file.
        # Capture the constructor diagnostic before treating its answers as a reference.
        with tempfile.TemporaryFile() as errors:
            original = os.dup(2)
            try:
                os.dup2(errors.fileno(), 2)
                handle = self.lib.Hunspell_create(bytes(aff), bytes(dic))
            finally:
                os.dup2(original, 2)
                os.close(original)
            errors.seek(0)
            self.diagnostic = errors.read().decode(errors="replace").replace(str(aff), aff.name).strip()
        if not handle:
            raise RuntimeError(f"could not load {aff.name}")
        try:
            encoding = self.lib.Hunspell_get_dic_encoding(handle).decode("ascii")
            return [bool(self.lib.Hunspell_spell(handle, word.encode(encoding))) for word in words]
        finally:
            self.lib.Hunspell_destroy(handle)


def answers(program, aff, dic, words):
    cmd = subprocess.run([program, "check", aff, dic], input="\n".join(words) + "\n",
                         text=True, capture_output=True, timeout=180)
    if cmd.returncode:
        return {"readError": cmd.stderr.strip().splitlines()[-1]}
    rows = [row.split("\t") for row in cmd.stdout.splitlines()]
    assert [row[0] for row in rows] == words and all(row[1] in ("0", "1") for row in rows)
    return [row[1] == "1" for row in rows]


def independent_grammars(folder, versions, oracle):
    folder.mkdir()
    aff, dic = folder/"case.aff", folder/"case.dic"
    words = ["work", "workable", "workables", "nework", "neworkable", "neworkables"]
    before = after = 0
    improved = []
    outcomes = bytearray()
    flags = lambda s: "/" + s if s else ""
    for cp, ci, co in itertools.product((0, 1), repeat=3):
        for kp, ki, ko in itertools.product("NY", repeat=3):
            for source in ("stem", "inner", "outer", "none"):
                aff.write_text(
                    f"SET UTF-8\nCIRCUMFIX c\nPFX P {kp} 1\nPFX P 0 ne{flags(cp * 'c')} .\n"
                    f"SFX A {ki} 1\nSFX A 0 able{flags('B' + ci * 'c' + (source == 'inner') * 'P')} .\n"
                    f"SFX B {ko} 1\nSFX B 0 s{flags(co * 'c' + (source == 'outer') * 'P')} .\n"
                )
                dic.write_text(f"1\nwork/A{(source == 'stem') * 'P'}\n")
                expected = oracle.check(aff, dic, words)
                assert not oracle.diagnostic, oracle.diagnostic
                outcomes.extend(expected)
                old, new = [answers(v, aff, dic, words) for v in versions]
                assert isinstance(old, list) and isinstance(new, list)
                for word, prev, got, want in zip(words, old, new, expected):
                    before += prev != want
                    after += got != want
                    assert prev != want or got == want, (aff.read_text(), dic.read_text(), word, prev, got, want)
                    if prev != got:
                        improved.append([cp, ci, co, kp + ki + ko, source, word, want])
    return {"grammars": 256, "wordsPerGrammar": words, "oracleSha256": hashlib.sha256(outcomes).hexdigest(),
            "baselineDifferences": before, "candidateDifferences": after, "corrected": improved}


def sample(affix, raw):
    charset = re.search(rb"(?mi)^SET[ \t]+([^\s]+)", affix)
    codec = charset.group(1).decode("ascii") if charset else "utf-8"
    lines = raw.decode(codec, errors="replace").splitlines()[1:]
    words = []
    for i in range(1, 33):
        start = len(lines) * i // 34
        for row in lines[start:start + 4000]:
            word = row.split("/", 1)[0].split("\t", 1)[0].strip()
            if 3 <= len(word) <= 18 and word.isalpha() and word.encode(codec, errors="ignore").decode(codec) == word:
                words.extend((word, word[::-1], word[1:], word + word[-1]))
                break
    return list(dict.fromkeys(words))


def counted_affix_copy(raw):
    """Recount actual rules for the independent reader. Change only the decimal header counts."""
    lines = raw.splitlines(keepends=True)
    groups = []
    current = None
    for i, line in enumerate(lines):
        fields = line.split(b"#", 1)[0].split()
        if not fields:
            continue
        if (len(fields) == 4 and fields[0] in (b"PFX", b"SFX")
                and fields[2] in (b"Y", b"N") and fields[3].isdigit()):
            current = [i, fields[:2], int(fields[3]), 0]
            groups.append(current)
        elif current is not None and fields[:2] == current[1]:
            current[3] += 1
        else:
            current = None
    changes = []
    for i, key, declared, count in groups:
        if declared == count:
            continue
        parts = re.match(rb"^(\s*\S+\s+\S+\s+[YN]\s+)([0-9]+)", lines[i])
        assert parts and int(parts[2]) == declared
        lines[i] = lines[i][:parts.start(2)] + str(count).encode() + lines[i][parts.end(2):]
        changes.append({"line": i + 1, "rule": b" ".join(key).decode("utf-8", "replace"),
                        "declared": declared, "actual": count})
    return b"".join(lines), changes


def published(versions, oracle, folder, tag, affix, dictionary):
    aff, dic = folder/"release.aff", folder/"release.dic"
    aff.write_bytes(affix)
    dic.write_bytes(dictionary)
    words = sample(affix, dictionary)
    assert words, tag
    result = {}
    for mode, n in [("single", 64), ("gen", 16)]:
        proc = subprocess.run([versions[0], mode, aff, dic, str(n), "1301"], text=True,
                              capture_output=True, timeout=120)
        if proc.returncode:
            result["generator_" + mode] = proc.stderr.strip().splitlines()[-1]
            break
        words.extend(proc.stdout.splitlines())
    words = list(dict.fromkeys(words))
    old, new = [answers(v, aff, dic, words) for v in versions]
    if isinstance(old, dict) or isinstance(new, dict):
        assert old == new, (tag, old, new)
        return {**result, **old, "probes": len(words)}
    expected = oracle.check(aff, dic, words)
    if oracle.diagnostic:
        assert "Failure loading aff file" in oracle.diagnostic, (tag, oracle.diagnostic)
        corrected, adjustments = counted_affix_copy(affix)
        assert adjustments, (tag, oracle.diagnostic)
        result["originalAffixDiagnostic"] = oracle.diagnostic
        result["originalReferenceDifferences"] = sum(x != y for x,y in zip(new,expected))
        result["referenceCopyHeaderCounts"] = adjustments
        reference = folder/"reference.aff"
        reference.write_bytes(corrected)
        expected = oracle.check(reference, dic, words)
        assert not oracle.diagnostic, (tag, oracle.diagnostic)
        result["countedReferenceAffixSha256"] = support["sha"](reference)
        assert aff.read_bytes() == affix
    for w, previous, candidate, reference in zip(words, old, new, expected):
        assert previous != reference or candidate == reference, (tag, w, previous, candidate, reference)
    differences = lambda values: sum(x != y for x, y in zip(values, expected))
    return {**result, "probes": len(words), "baselineDifferences": differences(old),
            "candidateDifferences": differences(new), "referenceAccepted": sum(expected),
            "remainingExamples": [[w, actual, ref] for w,actual,ref in zip(words,new,expected) if actual != ref],
            "answersSha256": [hashlib.sha256(bytes(data)).hexdigest() for data in (old, new, expected)]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "reference_library", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--release-packs", type=Path)
    args = parser.parse_args()
    versions = [args.baseline.resolve(), args.candidate.resolve()]
    oracle = Hunspell(args.reference_library.resolve())
    data = {"joyWordSha256": [support["sha"](x) for x in versions],
            "referenceLibrarySha256": support["sha"](args.reference_library)}
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory)
        data["threeAffixes"] = independent_grammars(path/"grammar", versions, oracle)
        if args.release_packs:
            index = args.release_packs/"index.json"
            assert support["sha"](index) == support["PUBLISHED_INDEX"]
            data["publishedIndexSha256"] = support["PUBLISHED_INDEX"]
            data["published"] = {}
            for row in json.loads(index.read_text())["packs"]:
                tag = row["locale"]
                archive = args.release_packs/(tag + ".tar.zst")
                assert archive.stat().st_size == row["size"] and support["sha"](archive) == row["sha256"]
                data["published"][tag] = published(versions, oracle, path, tag, *support["published_data"](archive))
    args.output.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
