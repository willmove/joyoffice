"""Compare timed suggestions from two Release builds of the craft-proof affix_check example."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import runpy
import statistics
import subprocess

support = runpy.run_path(str(Path(__file__).with_name("check-spelling-order.py")))


def measure(examples, folder, locale):
    aff, dic = [folder / locale / (locale + "." + ext) for ext in ("aff", "dic")]
    words = support["edit_probes"](aff.read_bytes(), dic.read_bytes())
    # Hold the loaded dictionary for a complete batch and exclude the warm-up.
    # Each invocation times just the query, including morph checking and ranking.
    times = [[], []]
    expected = None
    for block in range(4):
        for i in ((0, 1) if block % 2 == 0 else (1, 0)):
            inputs = words * 11
            run = subprocess.run(
                [examples[i], "suggest", aff, dic], input="\n".join(inputs) + "\n",
                text=True, capture_output=True, check=True, timeout=240,
            )
            lines = [row.split("\t", 2) for row in run.stdout.splitlines()]
            assert [row[0] for row in lines] == inputs
            for j in range(11):
                batch = lines[j * len(words):(j + 1) * len(words)]
                suggestions = [row[2] for row in batch]
                if expected is None:
                    expected = suggestions
                assert suggestions == expected, (locale, j, suggestions, expected)
                if j:
                    times[i].append(sum(int(row[1]) for row in batch) / 1000)
    return {
        "words": words,
        "suggestionsSha256": hashlib.sha256(json.dumps(expected, ensure_ascii=False).encode()).hexdigest(),
        "dictionarySha256": [support["sha"](path) for path in (aff, dic)],
        "batchMilliseconds": {
            label: {"samples": samples, "p50": statistics.median(samples),
                    "p95": sorted(samples)[math.ceil(0.95 * len(samples)) - 1]}
            for label, samples in zip(("baseline", "candidate"), times)
        },
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "dictionaries", "output"):
        parser.add_argument(name, type=Path)
    parser.add_argument("--locales", nargs="+", default=["nl", "pl", "es"])
    args = parser.parse_args()
    programs = [args.baseline.resolve(), args.candidate.resolve()]
    report = {"binarySha256": [support["sha"](p) for p in programs], "languages": {}}
    for locale in args.locales:
        report["languages"][locale] = measure(programs, args.dictionaries.resolve(), locale)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
