#!/usr/bin/env python3
"""Coverage-guided subprocess fuzzing of R8 DOCX and legacy journal decoders."""

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import random
import struct
import sys
import tempfile
import time
import zlib

ROOT = Path(__file__).resolve().parents[2]
LEGACY = ROOT / "experiments/docx-robustness"
sys.path.insert(0, str(LEGACY))
import driver as coverage

TARGETS = ("docx", "jwr-single", "jwr-slot")


def seal(body):
    return body + struct.pack("<I", zlib.crc32(body))


def journal_seeds():
    docx = (ROOT / "tests/fixtures/docx/DOCX-MIN-001.docx").read_bytes()
    text = "Legacy recovery 正文".encode()
    blob = lambda data: struct.pack("<Q", len(data)) + data
    payload = blob(docx) + blob(text)
    single = seal(b"JWREC001" + payload)
    slot = b"JWREC002" + struct.pack("<QBQ", 1, 0, 0)
    rich = b"JWREC003" + struct.pack("<QBQ", 2, 1, 7)
    discard = b"JWREC002" + struct.pack("<QBQ", 3, 2, 0)
    return [
        ("jwr-single", "v1", single),
        ("jwr-single", "v1-truncated", single[:-1]),
        ("jwr-slot", "v2", seal(slot + payload)),
        ("jwr-slot", "v3", seal(rich + payload + blob(docx))),
        ("jwr-slot", "discard", seal(discard)),
        ("jwr-slot", "wrong-slot", single),
    ]


def mutate_journal(data, mode, rng, iteration):
    # Exercise checksums as well as the fields behind them. Some mutations
    # specifically target lengths, text and the versioned header.
    if iteration % 5 == 0 or len(data) < 12:
        if not data or rng.randrange(2) == 0:
            return data[:rng.randrange(len(data) + 1)], "raw-truncate"
        body = bytearray(data)
        body[rng.randrange(len(body))] ^= rng.randrange(1, 256)
        return bytes(body), "raw-flip"
    body = bytearray(data[:-4])
    base = 8 if mode == "jwr-single" else 25
    choice = iteration % 5
    if choice == 1 and len(body) >= base + 8:
        length = struct.unpack_from("<Q", body, base)[0]
        sizes = (0, 1, length + 1, (96 << 20) + 1, (1 << 64) - 1)
        struct.pack_into("<Q", body, base, rng.choice(sizes) & ((1 << 64) - 1))
    elif choice == 2:
        del body[rng.randrange(8, len(body) + 1):]
    elif choice == 3:
        # Exercise generation/kind/version separately from UTF-8 and package bytes.
        fields = list(range(8, min(base, len(body))))
        if len(body) >= base + 8:
            text_at = base + 16 + struct.unpack_from("<Q", body, base)[0]
            if text_at < len(body):
                fields.append(text_at)
        offset = rng.choice(fields) if fields else rng.randrange(len(body))
        body[offset] ^= rng.randrange(1, 256)
    else:
        body.extend(rng.randbytes(rng.randrange(1, 9)))
    return seal(body[:coverage.MAX_INPUT - 4]), "recalculate-crc"


def invoke(binary, mode, data, timeout, memory_mib, env=None):
    result = coverage.run_process([str(binary), mode], data, timeout, memory_mib, env)
    if result["timed_out"]:
        return "timeout", result
    if result["exit"] not in (0, 2):
        return "crash", result
    accepted = result["exit"] == 0
    try:
        if (result["stdout_bytes"] <= coverage.MAX_OUTPUT
                and result["stderr_bytes"] <= coverage.MAX_OUTPUT
                and json.loads(result["stdout"]) == {
                    "target": mode, "sha256": coverage.sha(data), "accepted": accepted
                }):
            return "accepted" if accepted else "rejected", result
    except ValueError:
        pass
    return "protocol", result


def region_is_target(region, target):
    path = region[1]
    if target == "docx":
        return path.startswith(("crates/joyoffice-opc/", "crates/joyword-docx/"))
    return path == "crates/joyword-shell/src/legacy_recovery.rs"


def campaign(args):
    out = args.out.resolve()
    out.mkdir(parents=True)
    (out / "corpus").mkdir()
    chain = coverage.coverage_toolchain(args)
    memory = coverage.memory_capability()
    memory_mib = 768 if memory["enforced"] else 0
    rng = random.Random(args.seed)
    seeds = [
        ("docx", path.stem, coverage.bounded_read(path))
        for path in sorted((LEGACY / "seeds").glob("*.docx"))
    ] + journal_seeds()
    seen = {target: set() for target in TARGETS}
    corpus = {target: [] for target in TARGETS}
    saved_bytes = 0
    counts = {target: Counter() for target in TARGETS}
    profile_count = 0
    mutations = 0
    novel = Counter()
    mutators = Counter()
    baseline = {}
    failure = None

    def observe(target, label, data, origin):
        nonlocal profile_count, saved_bytes, failure
        raw = out / "input.profraw"
        raw.unlink(missing_ok=True)
        status, result = invoke(args.binary, target, data, args.timeout, memory_mib,
                                dict(os.environ, LLVM_PROFILE_FILE=str(raw)))
        counts[target][status] += 1
        if status not in ("accepted", "rejected"):
            path = out / "failure.input"
            path.write_bytes(data)
            (out / "failure.json").write_text(json.dumps({
                "target": target, "status": status, "label": label, "sha256": coverage.sha(data),
                "exit": result["exit"], "stderr": result["stderr"],
            }, indent=2), encoding="utf-8")
            if status == "crash":
                def same_failure(candidate):
                    observed, detail = invoke(args.binary, target, candidate, args.timeout, memory_mib,
                                              dict(os.environ, LLVM_PROFILE_FILE=str(raw)))
                    return status if (observed, detail["exit"]) == (status, result["exit"]) else observed + "-other"
                try:
                    minimized, _ = coverage.minimize(data, same_failure, status, max_runs=24)
                    (out / "minimized.input").write_bytes(minimized)
                except ValueError:
                    pass  # Keep the original evidence if the crash does not reproduce.
            failure = status
            return
        view = coverage.account_profile(chain["tools"], args.binary, raw, out / "coverage.profdata")
        if view is None:
            raise ValueError("missing LLVM profile; build the probe with -C instrument-coverage")
        profile_count += 1
        regions = {item for item in view["region_ids"] if region_is_target(item, target)}
        fresh = regions - seen[target]
        seen[target].update(fresh)
        if origin != "seed":
            mutators[origin] += 1
            if fresh:
                novel[target] += 1
        if (origin == "seed" or fresh) and sum(map(len, corpus.values())) < 128:
            if saved_bytes + len(data) <= 32 << 20:
                suffix = ".docx" if target == "docx" else ".jwr"
                (out / "corpus" / f"{target}-{label}{suffix}").write_bytes(data)
                corpus[target].append(data)
                saved_bytes += len(data)

    for target, label, data in seeds:
        observe(target, label, data, "seed")
        if failure:
            break
    if not failure:
        for target in TARGETS:
            if not seen[target] or not all(counts[target][kind] for kind in ("accepted", "rejected")):
                raise ValueError(f"{target}: seeds must cover decoder regions and both outcomes")
        baseline = {target: len(regions) for target, regions in seen.items()}
        end = time.monotonic() + args.seconds
        for iteration in range(args.iterations):
            if time.monotonic() >= end:
                break
            target = TARGETS[iteration % len(TARGETS)]
            pool = corpus[target]
            # Recent coverage discoveries are more likely to be mutated again.
            parent = pool[len(pool) - 1 - int(rng.random() ** 2 * len(pool))]
            if target == "docx":
                data, origin = coverage.mutate(parent, rng, iteration // len(TARGETS))
            else:
                data, origin = mutate_journal(parent, target, rng, iteration // len(TARGETS))
            observe(target, f"mutation-{iteration:04}", data, origin)
            mutations += 1
            if failure:
                break
    summary = {
        "feedback_signal": "llvm-source-regions",
        "toolchain": {key: value for key, value in chain.items() if key != "tools"},
        "probe_sha256": coverage.sha(args.binary.read_bytes()),
        "seed": args.seed,
        "profiles": profile_count,
        "mutations": mutations,
        "mutators": dict(mutators),
        "memory": dict(memory, limit_mib=memory_mib),
        "targets": {
            target: {
                "counts": dict(counts[target]),
                "seed_regions": baseline.get(target),
                "covered_regions": len(seen[target]),
                "novel_inputs": novel[target],
                "corpus_inputs": len(corpus[target]),
            } for target in TARGETS
        },
        "failure": failure,
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary))
    return int(bool(failure))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=lambda path: Path(path).resolve())
    parser.add_argument("--out", type=Path)
    parser.add_argument("--replay", nargs=2, metavar=("TARGET", "INPUT"))
    parser.add_argument("--iterations", type=int, default=120)
    parser.add_argument("--seconds", type=float, default=600)
    parser.add_argument("--timeout", type=float, default=8)
    parser.add_argument("--seed", type=int, default=20261104)
    parser.add_argument("--llvm-profdata", type=Path)
    parser.add_argument("--llvm-cov", type=Path)
    args = parser.parse_args()
    if args.replay:
        mode, path = args.replay
        if mode not in TARGETS:
            parser.error("unknown replay target")
        with tempfile.TemporaryDirectory() as tmp:
            env = dict(os.environ, LLVM_PROFILE_FILE=str(Path(tmp) / "replay.profraw"))
            status, result = invoke(args.binary, mode, coverage.bounded_read(path), args.timeout, 0, env)
        print(json.dumps({"status": status, **result}))
        return int(status not in ("accepted", "rejected"))
    if args.out is None or not (3 <= args.iterations <= 10000 and 0 < args.seconds <= 3600):
        parser.error("provide --out, 3..10000 iterations and (0, 3600] seconds")
    return campaign(args)


if __name__ == "__main__":
    sys.exit(main())
